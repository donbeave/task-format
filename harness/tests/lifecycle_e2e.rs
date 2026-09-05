//! Hermetic final-verification proof for lifecycle gate and promotion.
//!
//! This deliberately uses a local bare remote and the production gate/promotion code.  It does
//! not launch an agent or Docker: those are separate external prerequisites, not part of the
//! immutable-tree lifecycle invariant proved here.

use std::path::Path;
use std::process::Command;

use taskfmt::cmds::{self, Ctx};
use taskfmt::config::{ExperimentConfig, Resolved};
use taskfmt::interactive::Interaction;
use taskfmt::runstate::{ExperimentState, ExperimentTask, Manifest, SELFCHECK_PASS};

const TASK_README: &str = r#"---
schema: task/v5
id: TASK-900
title: "Prove exact lifecycle promotion"
kind: test
---

# TASK-900 — Prove exact lifecycle promotion

## Goal

The disposable lifecycle candidate is gated and promoted exactly once.

## Context

This package exists only to exercise the complete hermetic lifecycle proof.

## Preconditions

- **P-001:** A local bare Git remote exists.

## Scope

In scope:

- Immutable candidate gate and exact promotion.

Out of scope:

- Agent and container dispatch.

## Requirements

- **R-001 (MUST):** The lifecycle gate executes its proof body and promotes its recorded tree.

## Acceptance criteria

### AC-001 — Gate body and promotion complete

**Verification**

- **Type:** gate
- **Check:** `CHK-001`

## Fixed decisions

- **D-001:** Use a disposable file remote.

## Checklist

<!-- checklist:start -->
- [ ] **1** Verify.
    - [ ] **1.1** Gate and promote the exact candidate. (`R-001`, `AC-001`, `CHK-001`)
<!-- checklist:end -->
"#;

const VERIFY: &str = r#"schema = "verify/v2"
task_id = "TASK-900"
writable_paths = ["src/**"]

[[checks]]
id = "CHK-001"
phase = "gate"
shell = "mkdir -p proof && : > proof/body-executed && printf LIFECYCLE_BODY_EXECUTED"
requirements = ["R-001"]
acceptance = ["AC-001"]
expected = { stdout_contains = ["LIFECYCLE_BODY_EXECUTED"], required_artifacts = ["proof/body-executed"] }
"#;

const EXPERIMENT: &str = r#"schema = "experiment/v1"
[agents.default]
profile = "test"
[agents.profiles.test]
kind = "codex"
model = "test"
effort = "low"
image = "unused"
"#;

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("spawn git {args:?}: {error}"));
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn resolved(root: &Path) -> Resolved {
    let manifest = root.join("experiment.toml");
    std::fs::write(&manifest, EXPERIMENT).unwrap();
    let (config, root) = ExperimentConfig::load(&manifest).unwrap();
    Resolved::new(&root, config)
}

fn manifest(run_dir: &Path, repo_url: String, base_sha: String) -> Manifest {
    Manifest {
        run: "lifecycle-e2e-TASK-900".into(),
        run_dir: run_dir.display().to_string(),
        container: "hermetic-no-container".into(),
        agent: "test".into(),
        agent_kind: "codex".into(),
        model: "test".into(),
        effort: "low".into(),
        task: "TASK-900".into(),
        repo_url,
        base_sha: base_sha.clone(),
        clone_sha: base_sha,
        lifecycle_predecessor_sha: None,
        session_id: "hermetic".into(),
        pane: "none".into(),
        agent_name: "task".into(),
        start: "2026-09-06T00:00:00Z".into(),
        selfcheck: SELFCHECK_PASS.into(),
        experiment: Some("lifecycle-e2e".into()),
        gate: None,
        status_state: "GOAL_MET".into(),
        result_sha: None,
        pending_promotion_sha: None,
    }
}

#[test]
fn disposable_lifecycle_gate_promotes_the_recorded_tree_to_remote_main() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let resolved = resolved(root);
    let remote = root.join("remote.git");
    std::fs::create_dir_all(&remote).unwrap();
    git(&remote, &["init", "-q", "--bare", "-b", "main"]);

    let run_dir = root.join("run");
    let workspace = run_dir.join("workspace");
    let snapshot = run_dir.join("task-snapshot");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&snapshot).unwrap();
    std::fs::write(snapshot.join("README.md"), TASK_README).unwrap();
    std::fs::write(snapshot.join("verify.toml"), VERIFY).unwrap();
    let generated = taskfmt::progress::generate(&snapshot).unwrap();
    let completed_progress = generated.body.replace(
        "state: IN_PROGRESS\ncurrent: 1.1\nlatest_event: 1\n---\n\n## Events\n- 1 | STARTED | 1.1",
        "state: DONE\ncurrent: NONE\nlatest_event: 2\n---\n\n## Events\n- 1 | STARTED | 1.1\n- 2 | DONE | 1.1",
    );
    std::fs::create_dir_all(run_dir.join("progress")).unwrap();
    std::fs::write(run_dir.join("progress/progress.md"), completed_progress).unwrap();

    git(&workspace, &["init", "-q", "-b", "main"]);
    git(&workspace, &["config", "user.name", "Lifecycle Test"]);
    git(
        &workspace,
        &["config", "user.email", "lifecycle@test.invalid"],
    );
    std::fs::write(workspace.join("base.txt"), "base\n").unwrap();
    git(&workspace, &["add", "base.txt"]);
    git(&workspace, &["commit", "-qm", "base"]);
    let base = git(&workspace, &["rev-parse", "HEAD"]);
    git(
        &workspace,
        &[
            "remote",
            "add",
            "origin",
            &format!("file://{}", remote.display()),
        ],
    );
    git(&workspace, &["push", "-qu", "origin", "main"]);

    // This is the executor's completed work.  It is deliberately staged only by production
    // `gate_run`, which freezes its complete tree before executing the verifier body.
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::write(workspace.join("src/lifecycle.txt"), "candidate\n").unwrap();
    let mut run = manifest(
        &run_dir,
        format!("file://{}", remote.display()),
        base.clone(),
    );
    run.save(&run_dir).unwrap();

    let passed = cmds::gate::gate_run(&run_dir, &resolved, &mut run).unwrap();
    assert!(
        passed,
        "gate output must record a passing immutable candidate: {}",
        std::fs::read_to_string(run_dir.join("out/gate.log")).unwrap()
    );
    let gated = run.gate.as_ref().unwrap();
    assert!(gated.promotable());
    assert_eq!(gated.parent, base);
    assert!(!gated.candidate_tree.is_empty());
    let evidence: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&gated.matcher_evidence).unwrap()).unwrap();
    let body = evidence["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "CHK-001")
        .unwrap();
    assert_eq!(body["evidence"]["stdout"], "LIFECYCLE_BODY_EXECUTED");
    assert!(
        body["evidence"]["matchers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|matcher| {
                matcher["kind"] == "artifact.required"
                    && matcher["expected"] == "proof/body-executed"
                    && matcher["pass"] == true
            })
    );

    let ctx = Ctx {
        config_path: root.join("experiment.toml"),
        verbose: false,
        interaction: Interaction::new(true, true),
    };
    cmds::promote::promote_run(&ctx, &run_dir, true).unwrap();
    let promoted = Manifest::load(&run_dir).unwrap();
    let result = promoted.result_sha.as_ref().expect("promotion result SHA");
    let remote_head = git(&remote, &["rev-parse", "refs/heads/main"]);
    let remote_tree = git(&remote, &["rev-parse", "refs/heads/main^{tree}"]);
    assert_eq!(
        &remote_head, result,
        "remote main must name promoted commit"
    );
    assert_eq!(
        remote_tree,
        promoted.gate.as_ref().unwrap().candidate_tree,
        "remote main tree must equal the immutable tree the gate recorded"
    );

    // The lifecycle ledger is the durable complete-run claim, not the test's transient values.
    let mut state = ExperimentState::new("lifecycle-e2e", &promoted.repo_url);
    state.record_task(ExperimentTask {
        task: promoted.task.clone(),
        repo_url: promoted.repo_url.clone(),
        base_sha: promoted.base_sha.clone(),
        result_sha: promoted.result_sha.clone(),
        result_tree: Some(remote_tree),
        remote_main_sha: Some(remote_head),
        gate: promoted.gate.as_ref().unwrap().verdict.clone(),
        pushed: true,
        run_dir: run_dir.display().to_string(),
    });
    let state_file = root.join("experiment.json");
    state.save(&state_file).unwrap();
    let recorded = ExperimentState::load(&state_file).unwrap().unwrap();
    assert_eq!(recorded.passed_tasks(), ["TASK-900"]);
    let entry = &recorded.tasks[0];
    assert_eq!(
        entry.remote_main_sha.as_deref(),
        promoted.result_sha.as_deref()
    );
    assert_eq!(
        entry.result_tree.as_deref(),
        Some(promoted.gate.as_ref().unwrap().candidate_tree.as_str())
    );
}
