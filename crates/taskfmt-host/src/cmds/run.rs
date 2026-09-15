//! `taskfmt run --task TASK-101` — dispatch ONE task into ONE fresh, persistent, headed container.
//!
//! Pipeline: fresh clone → trusted overlay + base commit → task snapshot (+ template top-up) →
//! lint → gate selfcheck (opt-in `--selfcheck`; D13 nop + polarity, refuses on FAIL/NOVERDICT) →
//! agent-home preseed → `docker run -d --privileged` (no `--rm`) → entrypoint `taskfmt init` →
//! prereq wait → herdr pane → agent idle → prompt injection → goal-acceptance check → final manifest.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};

use crate::cmds::Ctx;
use crate::config::{DispatchOverrides, Resolved, resolve_dispatch, timestamp_compact};
use crate::ops::container::{self, SecretEnvFile};
use crate::ops::{docker, git, herdr};
use crate::runstate::{
    Manifest, SELFCHECK_FAIL, SELFCHECK_NOT_RUN, SELFCHECK_NOVERDICT, SELFCHECK_PASS, run_dir_name,
};
use taskfmt::executioncfg;
use taskfmt::redact;
use taskfmt::selfcheck::{self, Report, SelfcheckOpts};
use taskfmt::taskfile::TaskFile;
use taskfmt::verifycfg::{self, VerifyConfig};

pub const BASE_TAG: &str = "baseline";
/// `runs/<ID>/selfcheck.log`: the full D13 selfcheck report of the (opt-in) dispatch precondition.
pub const SELFCHECK_LOG: &str = "selfcheck.log";
const PREREQ_READY: &str = "/out/prereqs.ready";
const PREREQ_FAILED: &str = "/out/prereqs.FAILED";
const PANE_FILE: &str = "/out/pane-id";
const EMBEDDED_GOAL_PROMPT: &str = include_str!("../task-prompt.md");

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
    selfcheck: bool,
) -> anyhow::Result<i32> {
    let resolved = ctx.load()?;

    // A run tagged onto an existing experiment is pinned to that experiment's recorded repo, the
    // same rule `experiment --resume` follows. A repo is minted only when the experiment has no
    // state yet (first run under a fresh `--exp` id).
    let existing = match exp {
        Some(id) => crate::runstate::ExperimentState::load(&resolved.experiment_file(id))?,
        None => None,
    };
    let repo_url =
        crate::cmds::experiment::resolve_repo_url(existing.as_ref(), repo, |provided| {
            crate::cmds::repo::ensure_repo(ctx, &resolved, provided)
        })?;

    let outcome = dispatch_one(
        &resolved, agent, model, effort, task, &repo_url, None, exp, selfcheck,
    )?;

    if wait {
        let mut manifest = outcome.manifest;
        let status = wait_and_gate(&mut manifest, &outcome.run_dir, &resolved, kill_after)?;
        return Ok(run_wait_exit_code(&status, &manifest));
    }
    Ok(0)
}

/// Resolve profile/model/effort for one task without dispatching. Used by experiment confirmation.
pub fn plan_dispatch_for_task(
    resolved: &Resolved,
    task_id: &str,
    agent: Option<&str>,
    model: Option<&str>,
    effort: Option<&str>,
) -> anyhow::Result<(String, String, String)> {
    let location = crate::cmds::source::TaskLocation::resolve(&resolved.tasks_dir(), task_id)?;
    let execution = load_execution_for_task(&location.package_dir)?;
    let dispatch = resolve_dispatch(
        &resolved.cfg,
        execution.as_ref(),
        DispatchOverrides {
            agent,
            model,
            effort,
        },
    )?;
    Ok((dispatch.profile_name, dispatch.model, dispatch.effort))
}

fn load_execution_for_task(
    task_dir: &Path,
) -> anyhow::Result<Option<executioncfg::ExecutionConfig>> {
    executioncfg::ExecutionConfig::load_optional(task_dir)
        .map_err(|error| anyhow::anyhow!("{}:{}: {}", error.line, error.column, error.message))
}

/// Dispatch one task: everything up to and including prompt injection.
#[allow(clippy::too_many_arguments)]
pub fn dispatch_one(
    resolved: &Resolved,
    agent_override: Option<&str>,
    model_override: Option<&str>,
    effort_override: Option<&str>,
    task_id: &str,
    repo_url: &str,
    expected_predecessor: Option<&str>,
    exp: Option<&str>,
    selfcheck: bool,
) -> anyhow::Result<RunOutcome> {
    let cfg = &resolved.cfg;

    let location = crate::cmds::source::TaskLocation::resolve(&resolved.tasks_dir(), task_id)?;
    let execution = load_execution_for_task(&location.package_dir)?;
    let dispatch = resolve_dispatch(
        cfg,
        execution.as_ref(),
        DispatchOverrides {
            agent: agent_override,
            model: model_override,
            effort: effort_override,
        },
    )?;
    let profile_name = &dispatch.profile_name;
    let profile = dispatch.profile;
    let model = dispatch.model;
    let effort = dispatch.effort;

    docker::image_prerequisites(&profile.image).with_context(|| {
        format!(
            "cannot verify runtime prerequisites in {}; rebuild it with `taskfmt-host preload --auto` then `taskfmt-host build-images --agent all --auto`",
            profile.image
        )
    })?;

    // ---------- run dir ----------
    let run_id = run_dir_name(&timestamp_compact(), profile_name, &location.run_key);
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
    if let Some(expected) = expected_predecessor {
        anyhow::ensure!(
            clone_sha == expected,
            "lifecycle predecessor moved: origin/main is {clone_sha}, but recorded promoted predecessor is {expected}; resume refuses to change base"
        );
    }

    let source = location.load_contract()?;
    let task_dir = &source.package_dir;
    let task_id = source.contract_id.as_str();

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
    crate::ops::copy_tree(task_dir, &snapshot)?;
    top_up_snapshot(&snapshot, &resolved.template_dir())?;

    // ---------- 4. lint (aborts dispatch) ----------
    let report = taskfmt::lint::lint_path_with_experiment(task_dir, Some(cfg));
    crate::ops::write_file(&run_dir.join("lint.log"), &report.render())?;
    if !report.passed() {
        redact::emit_lines(report.render().lines());
        bail!(
            "task lint FAILED — not dispatching (log: {})",
            run_dir.join("lint.log").display()
        );
    }

    // ---------- 5. gate selfcheck (opt-in, aborts dispatch): D13 nop + polarity on the built workspace ----------
    let selfcheck_status = gate_selfcheck(&snapshot, &workspace, &base_sha, &run_dir, selfcheck)?;

    // Progress is created inside the container: entrypoint runs `taskfmt init` when missing.

    // ---------- 6. seed dir ----------
    let seed_dir = resolved.seed_dir();
    if seed_dir.is_dir() {
        crate::ops::copy_tree(&seed_dir, &run_dir.join("seed"))?;
    } else {
        redact::eemit(&format!(
            "note: no seed dir at {} — prereq-postgres starts unseeded",
            seed_dir.display()
        ));
    }

    // ---------- 8. prompt + session ----------
    let prompt = build_prompt(&profile.kind)?;
    redact::write_scrubbed(&run_dir.join("prompt.txt"), prompt.as_bytes())?;
    let session_id = uuid::Uuid::new_v4().to_string();

    // ---------- 9. agent home (profile carries resolved model/effort) ----------
    let agent_home = run_dir.join("agent-home");
    container::preseed_agent_home(&agent_home, &profile)?;

    // ---------- 10. container: named, persistent, detached, no -t (herdr server needs no TTY) ----------
    let agent_cmd = match profile.kind.as_str() {
        "claude" => container::claude_agent_cmd(&session_id, &model, &effort),
        "codex" => container::codex_agent_cmd(&model, &effort),
        "cursor" => container::cursor_agent_cmd(&model, &effort),
        other => bail!("unsupported agent kind: {other}"),
    };
    let mut manifest = Manifest {
        run: run_id.clone(),
        run_dir: run_dir.display().to_string(),
        container: container.clone(),
        agent: profile_name.clone(),
        agent_kind: profile.kind.clone(),
        model,
        effort,
        task: task_id.to_string(),
        repo_url: repo_url.to_string(),
        base_sha,
        clone_sha: clone_sha.clone(),
        lifecycle_predecessor_sha: expected_predecessor.map(str::to_string),
        session_id,
        pane: String::new(),
        agent_name: "task".to_string(),
        start: crate::config::timestamp_rfc3339(),
        selfcheck: selfcheck_status.to_string(),
        experiment: exp.map(str::to_string),
        gate: None,
        status_state: String::new(),
        result_sha: None,
        pending_promotion_sha: None,
    };

    let secrets = crate::ops::op::resolve_all(&profile.env_secret)?;
    let env_file = SecretEnvFile::create(&secrets)?;
    let mut cursor_auth_staging = None;
    // The recorded base SHA, not the movable `baseline` tag: the in-container `taskfmt verify` and the
    // host gate then share one immovable scope base (the tag stays for humans).
    let plan = container::launch_plan(
        cfg,
        resolved,
        &manifest,
        &profile,
        &agent_cmd,
        &manifest.base_sha,
        &mut cursor_auth_staging,
    )?;
    redact::emit(&format!(
        "== docker run {} (privileged, persistent, no --rm)",
        manifest.container
    ));
    // Persist identity before the Docker side effect. The Docker CLI has a short control timeout;
    // its daemon can create the named container after that CLI returns an error. This record makes
    // such a container locatable even if launch reconciliation has to report a failure.
    manifest.save(&run_dir)?;
    container::launch(&plan, &env_file)?;
    drop(env_file); // the 0600 env file is gone the moment the docker invocation returned

    // ---------- 11. prereq stage (inner dockerd + postgres + seeds) ----------
    let timeout = Duration::from_secs(cfg.runtime.prereq_timeout_s);
    redact::emit(&format!(
        "== waiting for the prereq stage (inner dockerd + postgres, up to {} s)",
        cfg.runtime.prereq_timeout_s
    ));
    wait_prereqs(&manifest, &run_dir, timeout)?;
    drop(cursor_auth_staging);

    // ---------- 12. herdr pane ----------
    manifest.pane = wait_pane(&manifest, Duration::from_secs(50))?;
    // rename before any `agent`-targeted call: "task" is the only stable target name, and the
    // agent does not exist under it until the rename lands
    herdr::rename_to_task(&manifest)?;
    manifest.save(&run_dir)?;

    // ---------- 13. readiness + prompt injection ----------
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
    if let Err(err) = herdr::inject_goal_prompt(&manifest, &prompt) {
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

/// `GateRecord::verdict` when the workspace changed while the gate was reading it. Distinct from
/// `fail` on purpose: a verdict computed over a mutating input is not a verdict about the work,
/// and recording it as `fail` charges the agent for the harness's timing (§6.1's `ABORT`, and
/// exactly what happened to TASK-002 on 2026-08-29).
pub const GATE_ABORT: &str = "abort";

/// `<run>/out/` — the terminal `Status` of record, written once, at gate time.
pub const STATUS_FILE: &str = "status.json";
/// `<run>/out/` — the workspace fingerprint taken either side of the gate.
pub const GATE_FINGERPRINT_FILE: &str = "gate-fingerprint.json";

/// Exit code for `run --wait`: terminal completion evidence plus a passing gate — not `GOAL_MET`
/// alone (Claude-specific). Aligns with `status::is_promotable` and the gate verdict.
pub(crate) fn run_wait_exit_code(status: &crate::cmds::status::Status, manifest: &Manifest) -> i32 {
    if crate::cmds::status::is_promotable(status)
        && manifest.gate.as_ref().is_some_and(|gate| gate.passed())
    {
        0
    } else {
        1
    }
}

/// `--wait`: poll until the run is terminal, **stop the agent**, then gate and record the verdict.
/// The gate record lands in the caller's manifest (and on disk) — the caller gates promotion on it.
///
/// The ordering is the fix, and each step is here for a reason that a rearrangement would break:
/// the status and the screen are captured while the container is still up, the container is then
/// stopped so the `/work` bind mount has no writer left, and the workspace is fingerprinted either
/// side of `gate_run` so that a tree which moves anyway is recorded as `abort` and never as `fail`.
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
    // The other half of the promotion decision, recorded beside the gate verdict: `gate_run` saves
    // the manifest, so the terminal state reaches disk through the write that already happens.
    manifest.status_state = status.state.clone();
    // Everything that needs a live container happens here, before the stop: the full status
    // (`status::check` still classifies a stopped container from artifact evidence — a verdict
    // or GOAL_RESULT — and only reports CONTAINER_STOPPED when there is none) and the screen
    // snapshot.
    let _ = redact::write_json(&run_dir.join("out").join(STATUS_FILE), &status);
    herdr::snapshot_screen(manifest, run_dir);
    quiesce(manifest)?;

    let workspace = run_dir.join("workspace");
    let before = workspace_fingerprint(&workspace);
    let mut passed = crate::cmds::gate::gate_run(run_dir, resolved, manifest)?;
    let after = workspace_fingerprint(&workspace);
    if let Some(reason) = tree_moved(&before, &after) {
        redact::eemit(&format!(
            "GATE ABORT {}: {reason} — the gate judged a moving tree, so this is not a verdict",
            manifest.run
        ));
        if let Some(gate) = manifest.gate.as_mut() {
            gate.verdict = GATE_ABORT.to_string();
        }
        manifest.save(run_dir)?;
        passed = false;
    }
    let _ = redact::write_json(
        &run_dir.join("out").join(GATE_FINGERPRINT_FILE),
        &serde_json::json!({ "before": before, "after": after }),
    );
    redact::emit(&format!(
        "GATE {} {}",
        manifest.run,
        manifest
            .gate
            .as_ref()
            .map(|gate| gate.verdict.to_uppercase())
            .unwrap_or_else(|| if passed { "PASS".into() } else { "FAIL".into() })
    ));
    Ok(status)
}

/// Stop the agent so that the tree the gate is about to judge cannot change under it.
///
/// `/goal clear` first, so herdr stops re-prompting and the agent is not killed mid-turn, then
/// `docker stop`, which is what actually removes the writer: the workspace is a bind mount and a
/// live container is a process with a handle on it. §6.1 already required this on the
/// `KILLED_TIMEOUT` path — "leaves the agent writing into the bind mount the gate is about to
/// read" — and the 2026-08-29 TASK-002 race is that same defect reached down the `IDLE` path, so
/// it is now the rule for every path into the gate rather than a special case of one.
///
/// The container is kept, never removed: `taskfmt attach` restarts a stopped one.
pub(crate) fn quiesce(manifest: &Manifest) -> anyhow::Result<()> {
    if docker::is_stopped(&manifest.container) {
        return Ok(());
    }
    if !docker::is_running(&manifest.container) {
        anyhow::bail!(
            "cannot prove container {} is quiesced; Docker inspect did not report running or stopped",
            manifest.container
        );
    }
    herdr::clear_goal(manifest);
    if docker::stop(&manifest.container, QUIESCE_GRACE_S) {
        redact::emit(&format!(
            "QUIESCED {} (stopped before gating; `taskfmt-host attach {}` restarts it)",
            manifest.container, manifest.run
        ));
        Ok(())
    } else {
        anyhow::bail!(
            "cannot prove container {} stopped; refusing to gate a possibly active workspace",
            manifest.container
        );
    }
}

/// docker's SIGTERM window before SIGKILL when quiescing: long enough for `script(1)` to flush
/// `tui.log`, short enough not to stall a chain.
const QUIESCE_GRACE_S: u64 = 20;

/// A cheap description of the judged tree: how many files it has and the newest mtime among them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WorkspaceFingerprint {
    pub files: usize,
    /// Newest mtime under the workspace, in nanoseconds since the epoch; 0 when there is none.
    pub newest_mtime_ns: u128,
}

/// Directory names the fingerprint never descends into, at any depth: build output and git's own
/// store, both of which the gate itself rewrites by running `cargo` and `git`.
const FINGERPRINT_SKIP_DIRS: [&str; 2] = ["target", ".git"];
/// File names the fingerprint ignores, for the same reason: `cargo` owns the lockfile and may
/// rewrite it on the gate's own first build. An agent write here is therefore not detected — a
/// stated residue, and the reason it is acceptable is that `quiesce` has already removed the
/// writer; this detector exists for the case where that failed.
const FINGERPRINT_SKIP_FILES: [&str; 1] = ["Cargo.lock"];

/// Fingerprint the workspace, or `None` when it cannot be read (never an abort: a missing answer
/// is not evidence of a change).
pub fn workspace_fingerprint(workspace: &Path) -> Option<WorkspaceFingerprint> {
    if !workspace.is_dir() {
        return None;
    }
    let mut files = 0usize;
    let mut newest = 0u128;
    for entry in walkdir::WalkDir::new(workspace)
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !(entry.file_type().is_dir() && FINGERPRINT_SKIP_DIRS.contains(&name.as_ref()))
        })
    {
        let Ok(entry) = entry else { return None };
        if entry.file_type().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if FINGERPRINT_SKIP_FILES.contains(&name.as_ref()) {
            continue;
        }
        files += 1;
        if let Ok(meta) = entry.metadata()
            && let Ok(mtime) = meta.modified()
            && let Ok(since) = mtime.duration_since(std::time::UNIX_EPOCH)
        {
            newest = newest.max(since.as_nanos());
        }
    }
    Some(WorkspaceFingerprint {
        files,
        newest_mtime_ns: newest,
    })
}

/// Did the judged tree change while the judge was reading it? `Some(reason)` only on a positive
/// observation: two fingerprints that were both taken and that differ.
pub fn tree_moved(
    before: &Option<WorkspaceFingerprint>,
    after: &Option<WorkspaceFingerprint>,
) -> Option<String> {
    let (before, after) = (before.as_ref()?, after.as_ref()?);
    if before == after {
        return None;
    }
    Some(format!(
        "the workspace changed during the gate ({} files, newest mtime {} -> {} files, newest mtime {})",
        before.files, before.newest_mtime_ns, after.files, after.newest_mtime_ns
    ))
}

/// Opt-in D13 dispatch precondition (`--selfcheck`): `taskfmt selfcheck` (nop + polarity; oracle
/// SKIPPED — no reference ships yet) against the task snapshot (trusted copies, `verify.toml`
/// topped up) and the freshly built workspace at the recorded base SHA. The workspace is never
/// mutated (scratch copy). Off by default: it runs the fixture's toolchain on the host, which is
/// container-only for the postgres-backed tasks — the container-mode selfcheck (inside the run
/// image, after the prereq stage) is pending. Returns the `Manifest::selfcheck` value.
fn gate_selfcheck(
    task_dir: &Path,
    workspace: &Path,
    base_sha: &str,
    run_dir: &Path,
    enabled: bool,
) -> anyhow::Result<&'static str> {
    if !enabled {
        redact::emit(
            "== gate selfcheck not run (opt in with --selfcheck; container-mode selfcheck pending)",
        );
        return Ok(SELFCHECK_NOT_RUN);
    }
    redact::emit(&format!(
        "== gate selfcheck (nop + polarity, base {base_sha})"
    ));
    let report = selfcheck::run(SelfcheckOpts {
        task_dir: task_dir.to_path_buf(),
        workspace: workspace.to_path_buf(),
        base: base_sha.to_string(),
        reference: None,
        keep: false,
    })
    .context("gate selfcheck")?;
    record_selfcheck(run_dir, &report)
}

/// `Manifest::selfcheck` vocabulary for a report that ran.
fn selfcheck_status(report: &Report) -> &'static str {
    if report.pass {
        SELFCHECK_PASS
    } else if report.noverdict {
        SELFCHECK_NOVERDICT
    } else {
        SELFCHECK_FAIL
    }
}

/// Pure part of [`gate_selfcheck`]: write `runs/<ID>/selfcheck.log`, echo the verdict lines, and
/// refuse to dispatch on `SELFCHECK RESULT FAIL` (the full report goes to stdout then). NOVERDICT
/// (a focused command not runnable on the host) refuses too — the package is unproven, not wrong.
fn record_selfcheck(run_dir: &Path, report: &Report) -> anyhow::Result<&'static str> {
    let log = run_dir.join(SELFCHECK_LOG);
    let text = report.render();
    crate::ops::write_file(&log, &text)?;
    let status = selfcheck_status(report);
    if status != SELFCHECK_PASS {
        redact::emit_lines(text.lines());
        let why = if status == SELFCHECK_NOVERDICT {
            "NOVERDICT (a focused command is not runnable on the host: toolchain missing?)"
        } else {
            "FAILED"
        };
        bail!(
            "gate selfcheck {why} — not dispatching (log: {}). Selfcheck is opt-in (--selfcheck) and runs on the host; the container-mode selfcheck is pending — dispatch without --selfcheck to proceed unproven",
            log.display()
        );
    }
    redact::emit_lines(
        text.lines()
            .filter(|line| line.starts_with("SELFCHECK ") && !line.starts_with("SELFCHECK work ")),
    );
    Ok(status)
}

/// Replace template task-id placeholders with the canonical id for this snapshot.
pub fn substitute_task_id(content: &str, task_id: &str) -> String {
    content
        .replace("TASK-000", task_id)
        .replace("<id-from-README>", task_id)
}

/// Resolve the task id from README frontmatter, falling back to an existing `verify.toml`.
pub fn resolve_snapshot_task_id(snapshot: &Path) -> anyhow::Result<String> {
    let readme = snapshot.join("README.md");
    if readme.is_file() {
        let text = std::fs::read_to_string(&readme)
            .with_context(|| format!("reading {}", readme.display()))?;
        if let Ok(tf) = TaskFile::parse(text, &readme) {
            return Ok(tf.frontmatter.id);
        }
    }
    let verify = snapshot.join(verifycfg::FILE_NAME);
    if verify.is_file() {
        let text = std::fs::read_to_string(&verify)
            .with_context(|| format!("reading {}", verify.display()))?;
        let cfg = VerifyConfig::parse_located(&text).map_err(|error| {
            anyhow::anyhow!("{}:{}: {}", verifycfg::FILE_NAME, error.line, error.message)
        })?;
        return Ok(cfg.task_id);
    }
    bail!(
        "cannot resolve task id from README.md frontmatter or {}",
        verifycfg::FILE_NAME
    )
}

/// Copy `AGENTS.md` and `verify.toml` from the template when the task package lacks them, plus the
/// `CLAUDE.md` → `AGENTS.md` sibling symlink (hard rule: never a real CLAUDE.md).
pub fn top_up_snapshot(snapshot: &Path, template_dir: &Path) -> anyhow::Result<()> {
    let task_id = resolve_snapshot_task_id(snapshot)?;
    for file in ["AGENTS.md", verifycfg::FILE_NAME] {
        if snapshot.join(file).exists() {
            continue;
        }
        let source = template_dir.join(file);
        if !source.is_file() {
            bail!("template is missing {file} ({})", source.display());
        }
        let template = std::fs::read_to_string(&source)
            .with_context(|| format!("reading template {}", source.display()))?;
        let materialized = substitute_task_id(&template, &task_id);
        crate::ops::write_file(&snapshot.join(file), &materialized)
            .with_context(|| format!("writing {} into the snapshot", file))?;
    }
    if !snapshot.join("CLAUDE.md").exists() {
        crate::ops::symlink(Path::new("AGENTS.md"), &snapshot.join("CLAUDE.md"))?;
    }
    Ok(())
}

/// The first ```` ```text ```` block of the embedded task-prompt.md whose info-string words name `kind`
/// (`text claude`, `text codex`, or a shared `text claude codex`), collapsed to one line (one
/// line avoids the `[Pasted text]` chip in the agent TUI).
pub fn build_prompt(kind: &str) -> anyhow::Result<String> {
    build_prompt_from_str(EMBEDDED_GOAL_PROMPT, kind)
        .with_context(|| "in embedded src/task-prompt.md")
}

/// Pure part of [`build_prompt`]: select + collapse, error when no block names `kind`.
pub fn build_prompt_from_str(text: &str, kind: &str) -> anyhow::Result<String> {
    if kind.is_empty() || kind.chars().any(char::is_whitespace) {
        bail!("invalid agent kind {kind:?}");
    }
    let mut lines: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if inside {
            if trimmed == "```" {
                break;
            }
            lines.push(line);
            continue;
        }
        if let Some(info) = trimmed.strip_prefix("```text")
            && info.split_whitespace().any(|word| word == kind)
        {
            inside = true;
        }
    }
    if !inside || lines.is_empty() {
        bail!("no ```text prompt block tagged for agent kind {kind:?}");
    }
    Ok(lines
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" "))
}

/// Poll `/out/prereqs.ready`; on `prereqs.FAILED` dump the markers, leave the container up, and
/// return the error the caller records.
fn wait_prereqs(manifest: &Manifest, run_dir: &Path, timeout: Duration) -> anyhow::Result<()> {
    let started = Instant::now();
    loop {
        if docker::read_file(&manifest.container, PREREQ_FAILED).is_some() {
            return Err(fail_prereqs(manifest, run_dir));
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
            return Err(fail_prereqs(manifest, run_dir));
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Hard rule: never leave the prereq stage without leaving the container up and the logs dumped.
/// Returns the error rather than ending the process: this runs three frames below an experiment
/// batch loop that owns durable state, and a non-local exit here skips every save that loop would
/// have performed.
fn fail_prereqs(manifest: &Manifest, run_dir: &Path) -> anyhow::Error {
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
    anyhow::anyhow!(
        "prereq stage FAILED — container {} left up for inspection",
        manifest.container
    )
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
/// (codex/cursor). Bracketed paste can leave a `[Pasted text]` chip without submitting — keep
/// sending Enter until herdr reports `working`/`blocked` or the unsubmitted marker clears.
fn confirm_acceptance(manifest: &Manifest, prompt: &str) -> anyhow::Result<()> {
    let transcript = crate::ops::transcript::claude_transcript(manifest);
    let prefix: String = prompt.chars().take(40).collect();
    let claude = manifest.agent_kind == "claude";
    let cursor = manifest.agent_kind == "cursor";
    let iterations = if claude { 15 } else { 30 };
    const SLEEP: Duration = Duration::from_secs(2);
    const ENTER_RETRIES_CODEX: [u32; 3] = [2, 5, 8];
    const ENTER_RETRIES_CURSOR: [u32; 6] = [1, 2, 3, 5, 8, 12];
    for iteration in 1..=iterations {
        std::thread::sleep(SLEEP);
        if claude
            && transcript.is_file()
            && crate::ops::transcript::has_any_goal_status(&transcript)
        {
            redact::emit("goal accepted (transcript sentinel)");
            return Ok(());
        }
        if cursor
            && herdr::pane_visible(manifest)
                .map(|screen| crate::cmds::status::screen_shows_active_goal(&screen))
                .unwrap_or(false)
        {
            redact::emit("goal accepted (/goal active on screen)");
            return Ok(());
        }
        if !claude {
            let unsubmitted = herdr::pane_visible(manifest)
                .map(|screen| crate::cmds::status::screen_shows_unsubmitted_prompt(&screen))
                .unwrap_or(false);
            match herdr::agent_status(manifest).as_deref() {
                Some("working") | Some("blocked") if !unsubmitted => {
                    redact::emit("prompt consumed (agent working or blocked)");
                    return Ok(());
                }
                _ => {}
            }
        }
        let enter_retries = if cursor {
            &ENTER_RETRIES_CURSOR[..]
        } else if !claude {
            &ENTER_RETRIES_CODEX[..]
        } else {
            &[][..]
        };
        if enter_retries.contains(&iteration)
            && herdr::pane_visible(manifest)
                .map(|screen| {
                    crate::cmds::status::screen_shows_unsubmitted_prompt(&screen)
                        || screen.contains(&prefix)
                })
                .unwrap_or(false)
        {
            redact::eemit("prompt visible but not submitted — sending Enter");
            herdr::send_enter(manifest);
        }
    }
    let timeout_s = (iterations as u64) * SLEEP.as_secs();
    redact::eemit(&format!(
        "warning: goal acceptance not confirmed within {timeout_s} s — attach and check: taskfmt-host attach {}",
        manifest.run
    ));
    Ok(())
}

fn print_summary(manifest: &Manifest, run_dir: &Path, prompt: &str) {
    let transcript_display = crate::cmds::status::transcript_display(manifest);
    redact::emit_lines([
        format!("run:        {}", run_dir.display()),
        format!(
            "container:  {}   (persistent; docker rm -f {} when done)",
            manifest.container, manifest.container
        ),
        format!(
            "attach:     taskfmt-host attach {}   (detach: ctrl+b q — never ctrl+c)",
            manifest.run
        ),
        format!("status:     taskfmt-host status {} [--wait]", manifest.run),
        format!("gate:       taskfmt-host gate {}", manifest.run),
        format!(
            "promote:    taskfmt-host promote {}  (only after GATE PASS)",
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

#[cfg(test)]
mod tests {
    use super::*;
    use taskfmt::selfcheck::Phase;

    fn touch(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn the_fingerprint_ignores_what_the_gate_itself_rewrites() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().join("workspace");
        touch(&ws.join("src/main.rs"), "fn main() {}\n");
        touch(&ws.join("Cargo.toml"), "[package]\n");
        let before = workspace_fingerprint(&ws);
        assert_eq!(before.as_ref().unwrap().files, 2);

        // everything cargo and git own during a gate run: build output, the git store, the lockfile
        touch(&ws.join("target/debug/pgtui"), "binary");
        touch(&ws.join("crates/pgtui/target/x"), "nested build output");
        touch(&ws.join(".git/index"), "gitstuff");
        touch(&ws.join("Cargo.lock"), "[[package]]\n");
        let after = workspace_fingerprint(&ws);
        assert_eq!(
            before, after,
            "the gate's own writes must not read as a change"
        );
        assert_eq!(tree_moved(&before, &after), None);
    }

    #[test]
    fn the_fingerprint_sees_a_source_file_the_agent_added_or_rewrote() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().join("workspace");
        touch(&ws.join("src/main.rs"), "fn main() {}\n");
        let before = workspace_fingerprint(&ws);

        // a new file: the count moves
        touch(&ws.join("src/store/mod.rs"), "pub fn open() {}\n");
        let added = workspace_fingerprint(&ws);
        assert_ne!(before, added);
        let reason = tree_moved(&before, &added).expect("an added file is a moved tree");
        assert!(reason.contains("changed during the gate"), "{reason}");

        // a rewrite of an existing file: the count holds and the newest mtime moves. This is the
        // 2026-08-29 TASK-002 shape exactly — `main.rs` gained `interactive_terminal` between
        // check 2 and check 3 of one gate run.
        let stamp = std::time::SystemTime::now() + std::time::Duration::from_secs(120);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(ws.join("src/main.rs"))
            .unwrap();
        file.set_modified(stamp).unwrap();
        let rewritten = workspace_fingerprint(&ws);
        assert_eq!(
            rewritten.as_ref().unwrap().files,
            added.as_ref().unwrap().files
        );
        assert!(
            tree_moved(&added, &rewritten).is_some(),
            "a rewrite is a moved tree"
        );
    }

    #[test]
    fn a_fingerprint_that_could_not_be_taken_is_never_an_abort() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(workspace_fingerprint(&dir.path().join("gone")), None);
        let some = Some(WorkspaceFingerprint {
            files: 1,
            newest_mtime_ns: 7,
        });
        // a missing answer is not evidence of a change: abort only on a positive observation
        assert_eq!(tree_moved(&None, &some), None);
        assert_eq!(tree_moved(&some, &None), None);
        assert_eq!(tree_moved(&None, &None), None);
        assert_eq!(tree_moved(&some, &some), None);
    }

    #[test]
    fn an_aborted_gate_is_not_a_passed_gate() {
        let record = crate::runstate::GateRecord {
            verdict: GATE_ABORT.to_string(),
            exit: 0,
            last_line: "RESULT PASS".into(),
            head: "head".into(),
            log: "/tmp/gate.log".into(),
            finished: "2026-08-29T19:57:59Z".into(),
            ..crate::runstate::GateRecord::default()
        };
        assert!(
            !record.passed(),
            "abort must never satisfy the promote refusal, even with exit 0"
        );
    }

    fn report(pass: bool, noverdict: bool) -> Report {
        let phase = |pass: bool, name: &str| Phase {
            pass,
            lines: vec![format!("SELFCHECK phase {name}")],
        };
        Report {
            header: vec!["SELFCHECK task t".into()],
            nop: phase(pass, "nop"),
            polarity: phase(pass, "polarity"),
            oracle: None,
            pass,
            noverdict,
            kept: None,
        }
    }

    #[test]
    fn selfcheck_fail_writes_log_and_refuses_dispatch() {
        let run_dir = tempfile::tempdir().unwrap();
        let err = record_selfcheck(run_dir.path(), &report(false, false)).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("gate selfcheck FAILED"), "{message}");
        assert!(message.contains(SELFCHECK_LOG), "{message}");
        assert!(
            message.contains("container-mode selfcheck is pending"),
            "{message}"
        );
        assert!(!message.contains("skip-selfcheck"), "{message}");
        let log = std::fs::read_to_string(run_dir.path().join(SELFCHECK_LOG)).unwrap();
        assert!(log.ends_with("SELFCHECK RESULT FAIL\n"), "{log}");
        assert!(log.contains("SELFCHECK oracle SKIPPED (no reference)"));
    }

    #[test]
    fn selfcheck_noverdict_writes_log_and_refuses_dispatch() {
        let run_dir = tempfile::tempdir().unwrap();
        let err = record_selfcheck(run_dir.path(), &report(false, true)).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("gate selfcheck NOVERDICT"), "{message}");
        assert!(message.contains("toolchain missing"), "{message}");
        assert!(message.contains(SELFCHECK_LOG), "{message}");
        assert!(run_dir.path().join(SELFCHECK_LOG).is_file());
        assert_eq!(selfcheck_status(&report(false, true)), SELFCHECK_NOVERDICT);
        assert_eq!(selfcheck_status(&report(false, false)), SELFCHECK_FAIL);
        assert_eq!(selfcheck_status(&report(true, false)), SELFCHECK_PASS);
    }

    #[test]
    fn selfcheck_pass_writes_log_and_dispatches() {
        let run_dir = tempfile::tempdir().unwrap();
        let status = record_selfcheck(run_dir.path(), &report(true, false)).unwrap();
        assert_eq!(status, SELFCHECK_PASS);
        let log = std::fs::read_to_string(run_dir.path().join(SELFCHECK_LOG)).unwrap();
        assert!(log.ends_with("SELFCHECK RESULT PASS\n"), "{log}");
    }

    #[test]
    fn selfcheck_is_opt_in_and_never_runs_nor_logs_by_default() {
        let run_dir = tempfile::tempdir().unwrap();
        // A non-existent task dir would be a missing input (66) if selfcheck ran.
        let status = gate_selfcheck(
            Path::new("/nonexistent/task"),
            Path::new("/nonexistent/workspace"),
            "deadbeef",
            run_dir.path(),
            false,
        )
        .unwrap();
        assert_eq!(status, SELFCHECK_NOT_RUN);
        assert!(!run_dir.path().join(SELFCHECK_LOG).exists());
        let err = gate_selfcheck(
            Path::new("/nonexistent/task"),
            Path::new("/nonexistent/workspace"),
            "deadbeef",
            run_dir.path(),
            true,
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("no such task dir"), "{err:#}");
    }

    const FIXTURE: &str = "# Launch prompt\n\nprose with ```text inline\n\n```sh\nclaude -p never\n```\n\n```text claude codex\n/goal Implement the task.\n  Done when:  verify   exits 0\nand DONE is printed.\n```\n\n```text codex\nunreachable second block\n```\n";

    /// The pre-kind extraction (first plain ```` ```text ```` fence, collapsed) — the claude
    /// prompt must stay byte-identical to what it produced.
    fn legacy_extract(text: &str) -> String {
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
        lines
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn shared_block_serves_both_kinds_and_matches_legacy_extraction() {
        let expected = "/goal Implement the task. Done when: verify exits 0 and DONE is printed.";
        let claude = build_prompt_from_str(FIXTURE, "claude").unwrap();
        let codex = build_prompt_from_str(FIXTURE, "codex").unwrap();
        assert_eq!(claude, expected);
        assert_eq!(codex, expected, "first matching block wins for codex too");
        let legacy_fixture = FIXTURE.replacen("```text claude codex", "```text", 1);
        assert_eq!(
            claude.as_bytes(),
            legacy_extract(&legacy_fixture).as_bytes(),
            "claude extraction must be byte-identical to the single-block extraction"
        );
    }

    #[test]
    fn split_blocks_diverge_by_kind() {
        let text = "```text claude\nclaude only\n```\n```text codex\ncodex only\n```\n";
        assert_eq!(
            build_prompt_from_str(text, "claude").unwrap(),
            "claude only"
        );
        assert_eq!(build_prompt_from_str(text, "codex").unwrap(), "codex only");
    }

    #[test]
    fn untagged_or_foreign_fences_are_never_injected() {
        let text = "```text\nplain\n```\n```sh\nclaude -p x\n```\n";
        let err = build_prompt_from_str(text, "claude").unwrap_err();
        assert!(err.to_string().contains("claude"), "{err:#}");
        assert!(build_prompt_from_str(FIXTURE, "ghost").is_err());
        assert!(build_prompt_from_str(FIXTURE, "").is_err());
        assert!(build_prompt_from_str("```text claude\n```\n", "claude").is_err());
    }

    #[test]
    fn embedded_goal_prompt_serves_claude_and_codex_under_the_cap() {
        let claude = build_prompt("claude").unwrap();
        let codex = build_prompt("codex").unwrap();
        assert_ne!(claude, codex);
        assert!(claude.starts_with("/goal "));
        assert!(!codex.starts_with("/goal "));
        assert!(claude.contains("`/task/README.md`"));
        assert!(claude.contains("`/task/AGENTS.md`"));
        assert!(codex.contains("`/task/README.md`"));
        assert!(codex.contains("GOAL_RESULT"));
        assert!(!claude.contains("@/task/README.md"));
        assert!(!claude.contains("@/task/AGENTS.md"));
        assert!(claude.contains("`taskfmt verify`"));
        assert!(claude.contains("after the last file change"));
        assert!(codex.contains("Goal achieved"));
        assert!(codex.contains("GOAL_RESULT task="));
        assert!(claude.contains("STATUS: INCOMPLETE"));
        assert!(
            claude.chars().count() <= 4_000,
            "{}",
            claude.chars().count()
        );
        assert!(codex.chars().count() <= 4_000, "{}", codex.chars().count());
        assert!(build_prompt("cursor").unwrap().starts_with("/goal "));
        assert!(build_prompt("ghost").is_err());
    }

    #[test]
    fn substitute_task_id_replaces_template_placeholders() {
        let input = "GOAL_PROGRESS task=TASK-000\nTASK: <id-from-README>\ntask_id = \"TASK-000\"";
        let out = substitute_task_id(input, "TASK-042");
        assert_eq!(
            out,
            "GOAL_PROGRESS task=TASK-042\nTASK: TASK-042\ntask_id = \"TASK-042\""
        );
    }

    #[test]
    fn top_up_snapshot_materializes_template_ids() {
        let root = tempfile::tempdir().unwrap();
        let snapshot = root.path().join("snapshot");
        let template = root.path().join("template");
        std::fs::create_dir_all(&snapshot).unwrap();
        std::fs::create_dir_all(&template).unwrap();
        std::fs::write(
            snapshot.join("README.md"),
            "---\nschema: task/v5\nid: TASK-042\ntitle: t\nkind: bugfix\n---\n",
        )
        .unwrap();
        std::fs::write(
            template.join("AGENTS.md"),
            "GOAL_PROGRESS task=<id-from-README>\nTASK: TASK-000\n",
        )
        .unwrap();
        std::fs::write(
            template.join("verify.toml"),
            "schema = \"verify/v2\"\ntask_id = \"TASK-000\"\nwritable_paths = [\"src\"]\n[[checks]]\nid = \"CHK-001\"\nphase = \"gate\"\nargv = [\"true\"]\nrequirements = [\"R-001\"]\nacceptance = [\"AC-001\"]\n",
        )
        .unwrap();

        top_up_snapshot(&snapshot, &template).unwrap();

        let agents = std::fs::read_to_string(snapshot.join("AGENTS.md")).unwrap();
        assert!(agents.contains("GOAL_PROGRESS task=TASK-042"));
        assert!(agents.contains("TASK: TASK-042"));
        assert!(!agents.contains("TASK-000"));
        assert!(!agents.contains("<id-from-README>"));

        let verify = std::fs::read_to_string(snapshot.join("verify.toml")).unwrap();
        assert!(verify.contains("task_id = \"TASK-042\""));
        assert!(!verify.contains("TASK-000"));

        assert!(snapshot.join("CLAUDE.md").exists());
    }

    #[test]
    fn top_up_snapshot_resolves_task_id_from_verify_when_readme_missing() {
        let root = tempfile::tempdir().unwrap();
        let snapshot = root.path().join("snapshot");
        let template = root.path().join("template");
        std::fs::create_dir_all(&snapshot).unwrap();
        std::fs::create_dir_all(&template).unwrap();
        std::fs::write(
            snapshot.join("verify.toml"),
            "schema = \"verify/v2\"\ntask_id = \"TASK-007\"\nwritable_paths = [\"src\"]\n[[checks]]\nid = \"CHK-001\"\nphase = \"gate\"\nargv = [\"true\"]\nrequirements = [\"R-001\"]\nacceptance = [\"AC-001\"]\n",
        )
        .unwrap();
        std::fs::write(template.join("AGENTS.md"), "TASK: TASK-000\n").unwrap();

        top_up_snapshot(&snapshot, &template).unwrap();

        let agents = std::fs::read_to_string(snapshot.join("AGENTS.md")).unwrap();
        assert_eq!(agents, "TASK: TASK-007\n");
    }

    /// `fail_prereqs` returns its failure instead of ending the process. The assertion is the
    /// test binary still being alive to make it: a process-terminating call here would take the
    /// whole harness down and `cargo test` would report the target as failed, not as red.
    ///
    /// No docker is needed. `docker::read_file` shells out and yields `None` when the exec fails,
    /// so the marker dump runs its whole body against a container no daemon knows.
    #[test]
    fn run_wait_exit_code_requires_promotable_status_and_gate_pass() {
        use crate::cmds::status::{GOAL_MET, IDLE, KILLED_TIMEOUT, REASON_GOAL_VERDICT};
        use crate::runstate::GateRecord;

        fn gate(verdict: &str) -> GateRecord {
            GateRecord {
                verdict: verdict.to_string(),
                exit: 0,
                last_line: "RESULT PASS".into(),
                head: "head".into(),
                log: "/tmp/gate.log".into(),
                finished: "2026-08-29T19:57:59Z".into(),
                ..GateRecord::default()
            }
        }
        fn manifest(gate: Option<GateRecord>) -> Manifest {
            Manifest {
                run: "r".into(),
                run_dir: "/tmp/r".into(),
                container: "c".into(),
                agent: "p".into(),
                agent_kind: "codex".into(),
                model: "m".into(),
                effort: "low".into(),
                task: "TASK-001".into(),
                repo_url: "u".into(),
                base_sha: "abc".into(),
                clone_sha: String::new(),
                lifecycle_predecessor_sha: None,
                session_id: "sid".into(),
                pane: "w1:p1".into(),
                agent_name: "task".into(),
                start: String::new(),
                selfcheck: SELFCHECK_NOT_RUN.into(),
                experiment: None,
                gate,
                status_state: String::new(),
                result_sha: None,
                pending_promotion_sha: None,
            }
        }
        fn status(state: &str, report_status: Option<&str>) -> crate::cmds::status::Status {
            crate::cmds::status::Status {
                state: state.to_string(),
                herdr_status: String::new(),
                goal_reason: String::new(),
                goal_result_line: report_status
                    .map(|_| "GOAL_RESULT task=TASK-001 status=DONE".into())
                    .unwrap_or_default(),
                report_status: report_status.map(str::to_string),
                goal_result_reject: None,
                goal_verdicts: None,
                terminal_reason: report_status.map(|_| REASON_GOAL_VERDICT.to_string()),
                transcript: String::new(),
                base_sha: "abc".into(),
                base_tag_ok: None,
                progress: None,
                native_goal_only: None,
            }
        }

        let passed = manifest(Some(gate("pass")));
        assert_eq!(
            run_wait_exit_code(&status(IDLE, Some("DONE")), &passed),
            0,
            "IDLE + anchored GOAL_RESULT + gate pass"
        );
        assert_eq!(
            run_wait_exit_code(&status(GOAL_MET, None), &manifest(Some(gate("fail")))),
            1,
            "GOAL_MET alone is not enough when the gate failed"
        );
        assert_eq!(
            run_wait_exit_code(&status(KILLED_TIMEOUT, None), &passed),
            1,
            "timeout is never success"
        );
    }

    #[test]
    fn fail_prereqs_returns_an_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("out")).unwrap();
        let manifest = Manifest {
            run: "r".into(),
            run_dir: dir.path().display().to_string(),
            container: "taskfmt-no-such-container-106".into(),
            agent: "p".into(),
            agent_kind: "claude".into(),
            model: "m".into(),
            effort: "low".into(),
            task: "TASK-001".into(),
            repo_url: "u".into(),
            base_sha: "abc".into(),
            clone_sha: String::new(),
            lifecycle_predecessor_sha: None,
            session_id: "sid".into(),
            pane: String::new(),
            agent_name: "task".into(),
            start: String::new(),
            selfcheck: SELFCHECK_NOT_RUN.into(),
            experiment: None,
            gate: None,
            status_state: String::new(),
            result_sha: None,
            pending_promotion_sha: None,
        };
        let err = fail_prereqs(&manifest, dir.path());
        println!("FAILPREREQS returned={err}");
        assert!(err.to_string().starts_with("prereq stage FAILED"), "{err}");
        assert!(err.to_string().contains(&manifest.container), "{err}");
    }
}
