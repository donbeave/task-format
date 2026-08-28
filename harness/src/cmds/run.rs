//! `taskfmt run --task TASK-101` — dispatch ONE task into ONE fresh, persistent, headed container.
//!
//! Pipeline: fresh clone → trusted overlay + base commit → task snapshot (+ template top-up) →
//! lint → progress-init → agent-home preseed → `docker run -d --privileged` (no `--rm`) → prereq
//! wait → herdr pane → agent idle → prompt injection → goal-acceptance check → manifest.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};

use crate::cmds::Ctx;
use crate::config::{Resolved, timestamp_compact};
use crate::ops::container::{self, SecretEnvFile};
use crate::ops::{docker, git, herdr};
use crate::redact;
use crate::runstate::{Manifest, run_dir_name};

pub const BASE_TAG: &str = "baseline";
const PREREQ_READY: &str = "/out/prereqs.ready";
const PREREQ_FAILED: &str = "/out/prereqs.FAILED";
const PANE_FILE: &str = "/out/pane-id";

pub struct RunOutcome {
    pub run_id: String,
    pub run_dir: PathBuf,
    pub manifest: Manifest,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    ctx: &Ctx,
    task: &str,
    repo: Option<&str>,
    agent: Option<&str>,
    model: Option<&str>,
    effort: Option<&str>,
    wait: bool,
    kill_after: Option<u64>,
    exp: Option<&str>,
) -> anyhow::Result<i32> {
    let resolved = ctx.load()?;
    let profile_name = agent
        .unwrap_or_else(|| resolved.cfg.default_profile())
        .to_string();
    let repo_url = crate::cmds::repo::ensure_repo(ctx, &resolved, repo)?;

    let outcome = dispatch_one(
        &resolved,
        &profile_name,
        model,
        effort,
        task,
        &repo_url,
        exp,
    )?;

    if wait {
        let mut manifest = outcome.manifest;
        let status = wait_and_gate(&mut manifest, &outcome.run_dir, &resolved, kill_after)?;
        return Ok(if status.state == crate::cmds::status::GOAL_MET {
            0
        } else {
            1
        });
    }
    Ok(0)
}

/// Dispatch one task: everything up to and including prompt injection.
pub fn dispatch_one(
    resolved: &Resolved,
    profile_name: &str,
    model_override: Option<&str>,
    effort_override: Option<&str>,
    task_id: &str,
    repo_url: &str,
    exp: Option<&str>,
) -> anyhow::Result<RunOutcome> {
    let cfg = &resolved.cfg;
    let profile = cfg.profile(profile_name)?.clone();
    let model = model_override.unwrap_or(&profile.model).to_string();
    let effort = effort_override.unwrap_or(&profile.effort).to_string();

    let task_dir = crate::cmds::resolve_task_arg(&resolved.tasks_dir(), task_id)?;
    if !task_dir.join("README.md").is_file() {
        bail!("{} has no README.md", task_dir.display());
    }

    // ---------- run dir ----------
    let run_id = run_dir_name(&timestamp_compact(), profile_name, task_id);
    let run_dir = resolved.runs_dir().join(&run_id);
    for dir in [
        "workspace",
        "task-snapshot",
        "progress",
        "agent-home",
        "out",
        "seed",
    ] {
        std::fs::create_dir_all(run_dir.join(dir))?;
    }
    let container = container::container_name(&run_id);

    // ---------- 1. fresh clone of the repo (origin/main) ----------
    let workspace = run_dir.join("workspace");
    redact::emit(&format!("== clone {repo_url}"));
    git::clone_main(repo_url, &workspace)
        .with_context(|| format!("cloning {repo_url} into {}", workspace.display()))?;
    let clone_sha = git::head(&workspace)?;

    // ---------- 2. trusted overlay + the trusted base commit (pushed with the task's chain) ----------
    let trusted = task_dir.join("trusted");
    if trusted.is_dir() {
        crate::ops::copy_tree_filtered(&trusted, &workspace, &|rel| rel != Path::new(".git"))?;
        git::add_all(&workspace)?;
    }
    let base_sha = if git::status_porcelain(&workspace)?.is_empty() {
        git::head(&workspace)?
    } else {
        git::commit(
            &workspace,
            &format!("planner: {task_id} trusted material"),
            true,
            false,
        )?
    };
    git::tag(&workspace, BASE_TAG)?;

    // ---------- 3. task snapshot, topped up from the template ----------
    let snapshot = run_dir.join("task-snapshot");
    crate::ops::copy_tree(&task_dir, &snapshot)?;
    top_up_snapshot(&snapshot, &resolved.template_dir())?;

    // ---------- 4. lint (aborts dispatch) + progress ----------
    let report = crate::lint::lint_path(&task_dir);
    crate::ops::write_file(&run_dir.join("lint.log"), &report.render())?;
    if !report.passed() {
        redact::emit_lines(report.render().lines());
        bail!(
            "task lint FAILED — not dispatching (log: {})",
            run_dir.join("lint.log").display()
        );
    }
    crate::cmds::progress_init::generate_and_write(
        &task_dir,
        Some(&run_dir.join("progress/progress.md")),
    )?;

    // ---------- 5. seed dir ----------
    let seed_dir = resolved.seed_dir();
    if seed_dir.is_dir() {
        crate::ops::copy_tree(&seed_dir, &run_dir.join("seed"))?;
    } else {
        redact::eemit(&format!(
            "note: no seed dir at {} — prereq-postgres starts unseeded",
            seed_dir.display()
        ));
    }

    // ---------- 6. prompt + session ----------
    let prompt = build_prompt(&resolved.goal_prompt())?;
    redact::write_scrubbed(&run_dir.join("prompt.txt"), prompt.as_bytes())?;
    let session_id = uuid::Uuid::new_v4().to_string();

    // ---------- 7. agent home ----------
    let agent_home = run_dir.join("agent-home");
    container::preseed_agent_home(&agent_home, &profile.kind)?;

    // ---------- 8. container: named, persistent, detached, no -t (herdr server needs no TTY) ----------
    let agent_cmd = match profile.kind.as_str() {
        "claude" => container::claude_agent_cmd(&session_id, &model, &effort),
        _ => container::codex_agent_cmd(&model, &effort),
    };
    let mut manifest = Manifest {
        run: run_id.clone(),
        run_dir: run_dir.display().to_string(),
        container: container.clone(),
        agent: profile_name.to_string(),
        agent_kind: profile.kind.clone(),
        model,
        effort,
        task: task_id.to_string(),
        repo_url: repo_url.to_string(),
        base_sha,
        clone_sha: clone_sha.clone(),
        session_id,
        pane: String::new(),
        agent_name: "task".to_string(),
        start: crate::config::timestamp_rfc3339(),
        experiment: exp.map(str::to_string),
        gate: None,
        result_sha: None,
    };

    let secrets = crate::ops::op::resolve_all(&profile.env_secret)?;
    let env_file = SecretEnvFile::create(&secrets)?;
    let plan = container::launch_plan(cfg, resolved, &manifest, &profile, &agent_cmd, BASE_TAG);
    redact::emit(&format!(
        "== docker run {} (privileged, persistent, no --rm)",
        manifest.container
    ));
    container::launch(&plan, &env_file)?;
    drop(env_file); // the 0600 env file is gone the moment the docker invocation returned

    // ---------- 9. prereq stage (inner dockerd + postgres + seeds) ----------
    let timeout = Duration::from_secs(cfg.runtime.prereq_timeout_s);
    redact::emit(&format!(
        "== waiting for the prereq stage (inner dockerd + postgres, up to {} s)",
        cfg.runtime.prereq_timeout_s
    ));
    wait_prereqs(&manifest, &run_dir, timeout)?;

    // ---------- 10. herdr pane ----------
    manifest.pane = wait_pane(&manifest, Duration::from_secs(50))?;
    // rename before any `agent`-targeted call: "task" is the only stable target name, and the
    // agent does not exist under it until the rename lands
    herdr::rename_to_task(&manifest)?;
    manifest.save(&run_dir)?;

    // ---------- 11. readiness + prompt injection ----------
    if !herdr::wait_idle(&manifest, 180_000)? {
        redact::eemit("agent not idle after 180 s (dialog? auth?). Screen:");
        if let Ok(screen) = herdr::pane_visible(&manifest) {
            redact::eemit(&screen);
        }
        bail!(
            "agent never became idle — container {} left up",
            manifest.container
        );
    }
    if let Err(err) = herdr::prompt(&manifest, &prompt) {
        redact::eemit(&format!("prompt refused: {err:#}"));
        if let Ok(screen) = herdr::pane_visible(&manifest) {
            redact::eemit(&screen);
        }
        bail!(
            "prompt was refused — container {} left up",
            manifest.container
        );
    }
    confirm_acceptance(&manifest, &prompt)?;
    manifest.save(&run_dir)?;

    print_summary(&manifest, &run_dir, &prompt);
    Ok(RunOutcome {
        run_id,
        run_dir,
        manifest,
    })
}

/// `--wait`: poll until the run is terminal, then gate and record the verdict. The gate record
/// lands in the caller's manifest (and on disk) — the caller gates promotion on it.
pub fn wait_and_gate(
    manifest: &mut Manifest,
    run_dir: &Path,
    resolved: &Resolved,
    kill_after_min: Option<u64>,
) -> anyhow::Result<crate::cmds::status::Status> {
    let minutes = kill_after_min.unwrap_or(resolved.cfg.runtime.kill_after_min);
    let status = crate::cmds::status::wait_terminal_state(
        manifest,
        run_dir,
        Duration::from_secs(60 * minutes),
    )?;
    let passed = crate::cmds::gate::gate_run(run_dir, resolved, manifest)?;
    redact::emit(&format!(
        "GATE {} {}",
        manifest.run,
        if passed { "PASS" } else { "FAIL" }
    ));
    Ok(status)
}

/// Copy `AGENTS.md` and `verify.toml` from the template when the task package lacks them, plus the
/// `CLAUDE.md` → `AGENTS.md` sibling symlink (hard rule: never a real CLAUDE.md).
pub fn top_up_snapshot(snapshot: &Path, template_dir: &Path) -> anyhow::Result<()> {
    for file in ["AGENTS.md", "verify.toml"] {
        if snapshot.join(file).exists() {
            continue;
        }
        let source = template_dir.join(file);
        if !source.is_file() {
            bail!("template is missing {file} ({})", source.display());
        }
        std::fs::copy(&source, snapshot.join(file))
            .with_context(|| format!("copying {} into the snapshot", source.display()))?;
    }
    if !snapshot.join("CLAUDE.md").exists() {
        crate::ops::symlink(Path::new("AGENTS.md"), &snapshot.join("CLAUDE.md"))?;
    }
    Ok(())
}

/// The ```text block of goal-prompt.md, collapsed to one line (one line avoids the
/// `[Pasted text]` chip in the agent TUI).
pub fn build_prompt(goal_prompt: &Path) -> anyhow::Result<String> {
    let text = std::fs::read_to_string(goal_prompt)
        .with_context(|| format!("cannot read {}", goal_prompt.display()))?;
    let mut lines: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        match line.trim() {
            "```text" if !inside => inside = true,
            "```" if inside => break,
            _ if inside => lines.push(line),
            _ => {}
        }
    }
    if lines.is_empty() {
        bail!("no ```text prompt block found in {}", goal_prompt.display());
    }
    Ok(lines
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" "))
}

/// Poll `/out/prereqs.ready`; on `prereqs.FAILED` dump the markers, leave the container up, exit 1.
fn wait_prereqs(manifest: &Manifest, run_dir: &Path, timeout: Duration) -> anyhow::Result<()> {
    let started = Instant::now();
    loop {
        if docker::read_file(&manifest.container, PREREQ_FAILED).is_some() {
            fail_prereqs(manifest, run_dir);
        }
        if docker::read_file(&manifest.container, PREREQ_READY).is_some() {
            let json =
                docker::read_file(&manifest.container, "/out/prereqs.json").unwrap_or_default();
            redact::emit("prereqs ready:");
            redact::emit_lines(json.lines());
            return Ok(());
        }
        if started.elapsed() >= timeout {
            redact::eemit(&format!("prereqs not ready after {} s", timeout.as_secs()));
            fail_prereqs(manifest, run_dir);
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Hard rule: never exit on prereq failure without leaving the container up and the logs dumped.
fn fail_prereqs(manifest: &Manifest, run_dir: &Path) -> ! {
    redact::eemit(&format!(
        "prereq stage FAILED — container {} left up for inspection",
        manifest.container
    ));
    for file in ["prereqs.json", "prereqs.log"] {
        let Some(body) = docker::read_file(&manifest.container, &format!("/out/{file}")) else {
            continue;
        };
        redact::eemit(&format!("--- {file} ---"));
        redact::eemit(&body);
        let path = run_dir.join("out").join(file);
        if let Ok(mut handle) = std::fs::File::create(&path) {
            let _ = handle.write_all(redact::scrub_bytes(body.as_bytes()).as_slice());
        }
    }
    let _ = std::io::stdout().flush();
    std::process::exit(1);
}

fn wait_pane(manifest: &Manifest, timeout: Duration) -> anyhow::Result<String> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Some(body) = docker::read_file(&manifest.container, PANE_FILE) {
            let pane = body.trim().to_string();
            if !pane.is_empty() {
                return Ok(pane);
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    redact::eemit(&format!(
        "no {PANE_FILE} after {} s — container {} left up (docker logs {})",
        timeout.as_secs(),
        manifest.container,
        manifest.container
    ));
    bail!("no pane id from the container");
}

/// Confirm the goal was accepted: transcript sentinel (claude) or the agent turning `working`
/// (codex). A prompt visible but unsubmitted gets an extra Enter at the 5th iteration.
fn confirm_acceptance(manifest: &Manifest, prompt: &str) -> anyhow::Result<()> {
    let transcript = crate::ops::transcript::claude_transcript(manifest);
    let prefix: String = prompt.chars().take(40).collect();
    let claude = manifest.agent_kind == "claude";
    for iteration in 1..=15 {
        std::thread::sleep(Duration::from_secs(2));
        if claude
            && transcript.is_file()
            && crate::ops::transcript::has_any_goal_status(&transcript)
        {
            redact::emit("goal accepted (transcript sentinel)");
            return Ok(());
        }
        if !claude && herdr::agent_status(manifest).as_deref() == Some("working") {
            redact::emit("prompt consumed (agent working)");
            return Ok(());
        }
        if iteration == 5
            && herdr::pane_visible(manifest)
                .map(|screen| screen.contains(&prefix))
                .unwrap_or(false)
        {
            redact::eemit("prompt visible but not submitted — resending Enter");
            herdr::send_enter(manifest);
        }
    }
    redact::eemit(&format!(
        "warning: goal acceptance not confirmed within 30 s — attach and check: taskfmt attach {}",
        manifest.run
    ));
    Ok(())
}

fn print_summary(manifest: &Manifest, run_dir: &Path, prompt: &str) {
    let transcript_display = if manifest.agent_kind == "claude" {
        format!(
            "{}/agent-home/projects/work/{}.jsonl",
            run_dir.display(),
            manifest.session_id
        )
    } else {
        format!(
            "{}/agent-home/sessions/**/rollout-*.jsonl",
            run_dir.display()
        )
    };
    redact::emit_lines([
        format!("run:        {}", run_dir.display()),
        format!(
            "container:  {}   (persistent; docker rm -f {} when done)",
            manifest.container, manifest.container
        ),
        format!(
            "attach:     taskfmt attach {}   (detach: ctrl+b q — never ctrl+c)",
            manifest.run
        ),
        format!("status:     taskfmt status {} [--wait]", manifest.run),
        format!("gate:       taskfmt gate {}", manifest.run),
        format!(
            "promote:    taskfmt promote {}  (only after GATE PASS)",
            manifest.run
        ),
        "raw log:    <run>/out/tui.log   (script(1) stream, from the first byte)".to_string(),
        format!("transcript: {transcript_display}"),
        "prereqs:    <run>/out/prereqs.json".to_string(),
        "progress:   <run>/progress/progress.md".to_string(),
        format!(
            "prompt:     {}… ({} chars, full text in prompt.txt)",
            prompt.chars().take(60).collect::<String>(),
            prompt.len()
        ),
    ]);
}
