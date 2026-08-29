//! `taskfmt promote <RUN>` — push a gated workspace. Refuses unless the manifest records a PASS
//! gate and the workspace HEAD is the gated HEAD.

use std::path::Path;

use anyhow::{Context, bail};

use crate::cmds::Ctx;
use crate::ops::git;
use crate::redact;
use crate::runstate::{GateRecord, Manifest};

pub fn run(ctx: &Ctx, run_id: &str, yes: bool) -> anyhow::Result<i32> {
    let (_, run_dir) = crate::cmds::load_run(ctx, run_id)?;
    promote_run(ctx, &run_dir, yes)?;
    Ok(0)
}

/// Promote one run. Any refusal happens before a push is attempted.
pub fn promote_run(ctx: &Ctx, run_dir: &Path, yes: bool) -> anyhow::Result<()> {
    let workspace = run_dir.join("workspace");
    if !workspace.is_dir() {
        bail!("no workspace at {}", workspace.display());
    }
    let mut manifest = Manifest::load(run_dir).context("reading the run manifest")?;

    // structural refusal: the manifest must already hold a PASS verdict
    let Some(gate) = manifest.gate.as_ref() else {
        bail!(
            "gate has not been run for {} — refusing to push (run `taskfmt gate` first)",
            manifest.run
        );
    };
    if !gate.passed() {
        bail!(
            "gate is {} for {} (last line {:?}) — never pushing a failed gate",
            gate.verdict,
            manifest.run,
            gate.last_line
        );
    }
    let head = git::head(&workspace)?;
    if head != gate.head {
        bail!(
            "workspace HEAD {head} differs from the gated HEAD {} — re-run `taskfmt gate` before promoting",
            gate.head
        );
    }
    if let Some(pushed) = manifest.result_sha.as_ref() {
        bail!("{} was already promoted ({pushed})", manifest.run);
    }

    // built ONCE and shown verbatim in the plan: the confirmation the operator answers is the
    // message that actually lands, not a second rendering of it that can drift.
    let message = commit_message(&manifest, gate, &title_of(run_dir));

    let mut plan = vec![
        format!("git -C {} add -A", workspace.display()),
        "git commit -s with the message:".to_string(),
    ];
    plan.extend(message.lines().map(|line| format!("    {line}")));
    plan.push(format!("git push origin main -> {}", manifest.repo_url));
    let interaction = crate::cmds::repo::subcommand_interaction(&ctx.interaction, yes);
    interaction
        .confirm(&format!("promote {}", manifest.run), &plan)?
        .or_decline("promoting")?;

    git::add_all(&workspace)?;
    if !git::status_porcelain(&workspace)?.is_empty() {
        git::commit(&workspace, &message, true, false)?;
    }
    let result_sha = git::head(&workspace)?;
    git::push(&workspace, "origin", "main").context("git push origin main")?;
    manifest.result_sha = Some(result_sha.clone());
    manifest.save(run_dir)?;
    redact::emit(&format!(
        "PROMOTE {} {} -> {}",
        manifest.run, result_sha, manifest.repo_url
    ));
    Ok(())
}

/// The commit message a promoted run pushes into the experiment repo: `<TASK>: <README title>`, a
/// blank line, then the provenance trailers.
///
/// Every value is READ from the run record — the profile, the model, the effort, the run id, the
/// gate verdict and the harness version. None of it is typed by an agent, which is the whole
/// point: an experiment commit has to name the model that produced it even though that model is
/// not the process making the commit, and a trailer an agent can compose is a trailer an agent can
/// get wrong. The e-mail is synthetic (`<agent_kind>@taskfmt.local`) because the author is a model
/// behind a CLI, not a mailbox; `agent_kind` is that CLI (`claude` / `codex`).
///
/// `Signed-off-by` is deliberately absent here: `git commit -s` appends it, and git appends into
/// this same trailer block, so the DCO line lands last without this function ever spelling it.
pub fn commit_message(manifest: &Manifest, gate: &GateRecord, title: &str) -> String {
    format!(
        "{task}: {title}\n\
         \n\
         Co-Authored-By: {model} <{kind}@taskfmt.local>\n\
         Taskfmt-Profile: {agent} effort={effort}\n\
         Taskfmt-Run: {run}\n\
         Taskfmt-Gate: {verdict} head={head}\n\
         Taskfmt-Version: {version}\n",
        task = manifest.task,
        model = manifest.model,
        kind = manifest.agent_kind,
        agent = manifest.agent,
        effort = manifest.effort,
        run = manifest.run,
        verdict = gate.verdict,
        head = gate.head,
        version = env!("CARGO_PKG_VERSION"),
    )
}

/// `<TASK>: <title>` from the trusted snapshot's README.
pub fn title_of(run_dir: &Path) -> String {
    let readme = run_dir.join("task-snapshot/README.md");
    std::fs::read_to_string(&readme)
        .ok()
        .and_then(|text| crate::taskfile::TaskFile::parse(text, &readme).ok())
        .map(|task| task.frontmatter.title)
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "task".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runstate::SELFCHECK_PASS;

    fn fixture() -> (Manifest, GateRecord) {
        let gate = GateRecord {
            verdict: "pass".into(),
            exit: 0,
            last_line: "DONE".into(),
            head: "def4560000000000000000000000000000000000".into(),
            log: "/tmp/run/out/gate.log".into(),
            finished: "2026-08-30T11:00:00Z".into(),
        };
        let manifest = Manifest {
            run: "20260830-101010-zai-flash-TASK-101".into(),
            run_dir: "/tmp/run".into(),
            container: "harness-20260830-101010-zai-flash-TASK-101".into(),
            agent: "zai-flash".into(),
            agent_kind: "claude".into(),
            model: "glm-5.3-flash".into(),
            effort: "low".into(),
            task: "TASK-101".into(),
            repo_url: "https://github.com/donbeave/x.git".into(),
            base_sha: "abc1230000000000000000000000000000000000".into(),
            clone_sha: "parent00000000000000000000000000000000000".into(),
            session_id: "00000000-0000-4000-8000-000000000000".into(),
            pane: "pane-1".into(),
            agent_name: "task".into(),
            start: "2026-08-30T10:10:10Z".into(),
            selfcheck: SELFCHECK_PASS.into(),
            experiment: Some("EXP-1".into()),
            gate: Some(gate.clone()),
            status_state: "GOAL_MET".into(),
            result_sha: None,
        };
        (manifest, gate)
    }

    /// The promoted message names the model that did the work, and names it from the run record.
    /// Asserted line by line rather than with `contains`, because a trailer git will parse has to
    /// be the WHOLE line: a substring match passes on a line that also carries something else.
    #[test]
    fn promote_commit_message_attributes_the_model_from_the_run_record() {
        let (manifest, gate) = fixture();
        let message = commit_message(&manifest, &gate, "Fix the trusted screen test");
        println!("PROMOTE-MESSAGE-BEGIN\n{message}PROMOTE-MESSAGE-END");

        let lines: Vec<&str> = message.lines().collect();
        assert_eq!(
            lines,
            vec![
                "TASK-101: Fix the trusted screen test",
                "",
                "Co-Authored-By: glm-5.3-flash <claude@taskfmt.local>",
                "Taskfmt-Profile: zai-flash effort=low",
                "Taskfmt-Run: 20260830-101010-zai-flash-TASK-101",
                "Taskfmt-Gate: pass head=def4560000000000000000000000000000000000",
                &format!("Taskfmt-Version: {}", env!("CARGO_PKG_VERSION")),
            ]
        );
    }

    /// Two guards on the shape rather than the content. The blank line after the subject is what
    /// makes the rest a body at all; without it git reads the trailers as a continuation of the
    /// subject. And the DCO line must NOT be typed here — `git commit -s` appends exactly one, so
    /// spelling it as well would push a commit carrying it twice.
    #[test]
    fn promote_commit_message_leaves_the_signoff_to_git() {
        let (manifest, gate) = fixture();
        let message = commit_message(&manifest, &gate, "Fix the trusted screen test");
        assert!(
            message.starts_with("TASK-101: Fix the trusted screen test\n\n"),
            "subject must be followed by a blank line: {message:?}"
        );
        assert!(
            !message.contains("Signed-off-by"),
            "git commit -s appends the DCO line; this function must not: {message:?}"
        );
        assert!(
            message.ends_with('\n'),
            "the trailer block ends with a newline so git appends into it: {message:?}"
        );
    }

    /// The e-mail's local part follows the agent CLI, so a codex-driven run is not mislabelled as
    /// a claude one. The model is the display name in both cases.
    #[test]
    fn promote_commit_message_follows_the_agent_kind_for_the_address() {
        let (mut manifest, gate) = fixture();
        manifest.agent_kind = "codex".into();
        manifest.model = "gpt-6-mini".into();
        manifest.agent = "openai-mini".into();
        manifest.effort = "high".into();
        let message = commit_message(&manifest, &gate, "Something else");
        println!("PROMOTE-MESSAGE-CODEX-BEGIN\n{message}PROMOTE-MESSAGE-CODEX-END");
        assert!(
            message
                .lines()
                .any(|line| line == "Co-Authored-By: gpt-6-mini <codex@taskfmt.local>"),
            "{message:?}"
        );
        assert!(
            message
                .lines()
                .any(|line| line == "Taskfmt-Profile: openai-mini effort=high"),
            "{message:?}"
        );
    }
}
