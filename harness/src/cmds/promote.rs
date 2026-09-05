//! `taskfmt promote <RUN>` — create and push precisely the immutable tree a gate recorded.

use std::path::Path;

use anyhow::bail;

use crate::cmds::Ctx;
use crate::runstate::{GateRecord, Manifest};

pub fn run(ctx: &Ctx, run_id: &str, yes: bool) -> anyhow::Result<i32> {
    let (_, run_dir) = crate::cmds::load_run(ctx, run_id)?;
    promote_run(ctx, &run_dir, yes)?;
    Ok(0)
}

/// Promote one run. Any refusal happens before a push is attempted.
pub fn promote_run(_ctx: &Ctx, run_dir: &Path, _yes: bool) -> anyhow::Result<()> {
    let workspace = run_dir.join("workspace");
    if !workspace.is_dir() {
        bail!("no workspace at {}", workspace.display());
    }
    let mut manifest = Manifest::load(run_dir)?;
    let gate = manifest
        .gate
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("promotion refused: no gate record"))?;
    if !gate.promotable() {
        bail!(
            "promotion refused: gate is not a complete immutable passing record (schema={} verdict={} terminal={})",
            gate.schema,
            gate.verdict,
            gate.terminal_state
        );
    }
    if manifest.result_sha.is_some() {
        bail!("promotion refused: run already has a result commit");
    }

    // `commit-tree` consumes recorded object IDs directly. Moving local branches or a dirty
    // worktree cannot alter this commit; the remote lease rejects a concurrently changed main.
    let commit = match manifest.pending_promotion_sha.clone() {
        Some(commit) => commit,
        None => {
            let message = commit_message(&manifest, gate, &title_of(run_dir));
            let commit = crate::ops::git::commit_tree(
                &workspace,
                &gate.candidate_tree,
                &gate.parent,
                &message,
                true,
            )?;
            manifest.pending_promotion_sha = Some(commit.clone());
            manifest.save(run_dir)?;
            commit
        }
    };
    if crate::ops::git::remote_main(&workspace, "origin")?.as_deref() == Some(commit.as_str()) {
        manifest.result_sha = Some(commit);
        manifest.pending_promotion_sha = None;
        manifest.save(run_dir)?;
        return Ok(());
    }
    crate::ops::git::push_main_with_lease(&workspace, "origin", &commit, &gate.parent)?;
    manifest.result_sha = Some(commit);
    manifest.pending_promotion_sha = None;
    manifest.save(run_dir)?;
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
         Taskfmt-Gate: {verdict} tree={tree} parent={parent}\n\
         Taskfmt-Version: {version}\n",
        task = manifest.task,
        model = manifest.model,
        kind = manifest.agent_kind,
        agent = manifest.agent,
        effort = manifest.effort,
        run = manifest.run,
        verdict = gate.verdict,
        tree = gate.candidate_tree,
        parent = gate.parent,
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
    use std::process::Command;

    fn fixture() -> (Manifest, GateRecord) {
        let gate = GateRecord {
            verdict: "pass".into(),
            exit: 0,
            last_line: "DONE".into(),
            head: "def4560000000000000000000000000000000000".into(),
            candidate_tree: "tree789000000000000000000000000000000000".into(),
            parent: "parent00000000000000000000000000000000000".into(),
            log: "/tmp/run/out/gate.log".into(),
            finished: "2026-08-30T11:00:00Z".into(),
            ..GateRecord::default()
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
            lifecycle_predecessor_sha: None,
            session_id: "00000000-0000-4000-8000-000000000000".into(),
            pane: "pane-1".into(),
            agent_name: "task".into(),
            start: "2026-08-30T10:10:10Z".into(),
            selfcheck: SELFCHECK_PASS.into(),
            experiment: Some("EXP-1".into()),
            gate: Some(gate.clone()),
            status_state: "GOAL_MET".into(),
            result_sha: None,
            pending_promotion_sha: None,
        };
        (manifest, gate)
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        crate::ops::git::output(Command::new("git").current_dir(dir).args(args))
            .unwrap()
            .trim()
            .to_string()
    }

    /// A complete on-disk run plus a bare `origin/main`. The candidate is deliberately left as a
    /// tree object rather than a branch commit: promotion must reconstruct from this exact object.
    fn gated_run() -> (
        tempfile::TempDir,
        std::path::PathBuf,
        std::path::PathBuf,
        String,
        String,
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let bare = tmp.path().join("remote.git");
        std::fs::create_dir_all(&bare).unwrap();
        git(&bare, &["init", "-q", "--bare", "-b", "main"]);

        let run_dir = tmp.path().join("run");
        let workspace = run_dir.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        crate::ops::git::init(&workspace).unwrap();
        std::fs::write(workspace.join("base.txt"), "base\n").unwrap();
        crate::ops::git::add_all(&workspace).unwrap();
        let parent = crate::ops::git::commit(&workspace, "base", true, false).unwrap();
        crate::ops::git::remote_add(&workspace, "origin", &format!("file://{}", bare.display()))
            .unwrap();
        crate::ops::git::push_upstream(&workspace, "origin", "main").unwrap();

        std::fs::write(workspace.join("candidate.txt"), "gated\n").unwrap();
        crate::ops::git::add_all_including_ignored(&workspace).unwrap();
        let tree = crate::ops::git::write_tree(&workspace).unwrap();
        std::fs::create_dir_all(run_dir.join("task-snapshot")).unwrap();
        std::fs::write(
            run_dir.join("task-snapshot/README.md"),
            "---\ntitle: Exact promotion\n---\n",
        )
        .unwrap();

        let (mut manifest, mut gate) = fixture();
        manifest.run_dir = run_dir.display().to_string();
        gate.schema = "gate/v3".into();
        gate.candidate_tree = tree.clone();
        gate.parent = parent.clone();
        gate.task_sha256 = "task-digest".into();
        gate.verifier_sha256 = "verifier-digest".into();
        gate.harness_fingerprint = "harness-digest".into();
        gate.evidence_sha256 = "evidence-digest".into();
        gate.matcher_evidence_sha256 = "matcher-evidence-digest".into();
        gate.matcher_evidence = "/tmp/run/out/gate-evidence.json".into();
        gate.terminal_state = "GOAL_MET".into();
        manifest.gate = Some(gate);
        manifest.save(&run_dir).unwrap();
        (tmp, run_dir, bare, parent, tree)
    }

    #[test]
    fn promotion_uses_the_recorded_tree_after_local_main_moves() {
        let (_tmp, run_dir, bare, _parent, tree) = gated_run();
        let workspace = run_dir.join("workspace");

        // This moves local main to a different tree after the candidate was recorded.
        git(&workspace, &["reset", "--hard", "HEAD"]);
        std::fs::write(workspace.join("distraction.txt"), "later local work\n").unwrap();
        crate::ops::git::add_all(&workspace).unwrap();
        crate::ops::git::commit(&workspace, "local distraction", true, false).unwrap();
        promote_fixture(&run_dir).unwrap();

        let pushed = crate::ops::git::head(&bare).unwrap();
        assert_eq!(
            git(&bare, &["rev-parse", &format!("{pushed}^{{tree}}")]),
            tree,
            "remote main must contain the gate's tree, not later local main"
        );
        let manifest = Manifest::load(&run_dir).unwrap();
        assert_eq!(manifest.result_sha.as_deref(), Some(pushed.as_str()));
    }

    #[test]
    fn promotion_refuses_when_the_remote_main_lease_changed() {
        let (_tmp, run_dir, bare, _parent, _tree) = gated_run();
        let racer = run_dir.parent().unwrap().join("racer");
        crate::ops::git::clone_main(&format!("file://{}", bare.display()), &racer).unwrap();
        std::fs::write(racer.join("race.txt"), "race\n").unwrap();
        crate::ops::git::add_all(&racer).unwrap();
        crate::ops::git::commit(&racer, "race", true, false).unwrap();
        crate::ops::git::push(&racer, "origin", "main").unwrap();
        let raced_head = crate::ops::git::head(&bare).unwrap();

        let error = promote_fixture(&run_dir).unwrap_err().to_string();
        assert!(error.contains("git push with main lease failed"), "{error}");
        assert_eq!(crate::ops::git::head(&bare).unwrap(), raced_head);
        assert!(Manifest::load(&run_dir).unwrap().result_sha.is_none());
    }

    #[test]
    fn promotion_refuses_an_incomplete_gate_record() {
        let (_tmp, run_dir, _bare, _parent, _tree) = gated_run();
        let mut manifest = Manifest::load(&run_dir).unwrap();
        let gate = manifest.gate.as_mut().unwrap();
        gate.evidence_sha256.clear();
        manifest.save(&run_dir).unwrap();

        let error = promote_fixture(&run_dir).unwrap_err().to_string();
        assert!(
            error.contains("not a complete immutable passing record"),
            "{error}"
        );
        assert!(Manifest::load(&run_dir).unwrap().result_sha.is_none());

        let mut manifest = Manifest::load(&run_dir).unwrap();
        manifest.gate.as_mut().unwrap().evidence_sha256 = "evidence-digest".into();
        manifest
            .gate
            .as_mut()
            .unwrap()
            .matcher_evidence_sha256
            .clear();
        manifest.save(&run_dir).unwrap();
        let error = promote_fixture(&run_dir).unwrap_err().to_string();
        assert!(
            error.contains("not a complete immutable passing record"),
            "{error}"
        );
    }

    #[test]
    fn promotion_refuses_a_nonpromotable_terminal_gate_record() {
        let (_tmp, run_dir, _bare, _parent, _tree) = gated_run();
        let mut manifest = Manifest::load(&run_dir).unwrap();
        manifest.gate.as_mut().unwrap().terminal_state = "AGENT_EXITED".into();
        manifest.save(&run_dir).unwrap();

        let error = promote_fixture(&run_dir).unwrap_err().to_string();
        assert!(
            error.contains("not a complete immutable passing record"),
            "{error}"
        );
        assert!(Manifest::load(&run_dir).unwrap().result_sha.is_none());
    }

    #[test]
    fn promotion_refuses_terminal_progress_without_a_gate_record() {
        let (_tmp, run_dir, _bare, _parent, _tree) = gated_run();
        let mut manifest = Manifest::load(&run_dir).unwrap();
        manifest.gate = None;
        manifest.save(&run_dir).unwrap();

        // A completed progress stream is coordination data. It deliberately cannot stand in
        // for gate identity/evidence fields, so it never authorizes a push.
        std::fs::create_dir_all(run_dir.join("progress")).unwrap();
        std::fs::write(
            run_dir.join("progress/progress.md"),
            "---\nschema: progress/v1\ntask: TASK-042\nstate: DONE\ncurrent: NONE\nlatest_event: 10\n---\n\n## Events\n- 1 | STARTED | 1.1\n- 2 | DONE | 1.1\n- 3 | STARTED | 2.1\n- 4 | DONE | 2.1\n- 5 | STARTED | 2.2\n- 6 | DONE | 2.2\n- 7 | STARTED | 2.3\n- 8 | DONE | 2.3\n- 9 | STARTED | 3.1\n- 10 | DONE | 3.1\n\n## Handoff\nCURRENT_FAILURE: none\n",
        )
        .unwrap();

        let error = promote_fixture(&run_dir).unwrap_err().to_string();
        assert!(
            error.contains("promotion refused: no gate record"),
            "{error}"
        );
        assert!(Manifest::load(&run_dir).unwrap().result_sha.is_none());
    }

    fn promote_fixture(run_dir: &Path) -> anyhow::Result<()> {
        let ctx = Ctx {
            config_path: std::path::PathBuf::new(),
            verbose: false,
            interaction: crate::interactive::Interaction::new(true, true),
        };
        promote_run(&ctx, run_dir, true)
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
                "Taskfmt-Gate: pass tree=tree789000000000000000000000000000000000 parent=parent00000000000000000000000000000000000",
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
