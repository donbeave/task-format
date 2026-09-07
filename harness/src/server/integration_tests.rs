//! End-to-end HTTP scheduling through the production process adapter and immutable gate.
//! Only the agent is substituted: a child test process produces deterministic candidate work.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use super::{AppState, ServerConfig, adapter::AdapterConfig, router};
use crate::runstate::{Manifest, SELFCHECK_PASS};

const README: &str = r#"---
schema: task/v5
id: TASK-900
title: "Verify monitor execution"
kind: test
---

# TASK-900 — Verify monitor execution

## Goal

Verify the monitor runs the immutable candidate gate.

## Context

Disposable execution fixture.

## Preconditions

- **P-001:** A local Git repository exists.

## Scope

In scope:

- Gate verification.

Out of scope:

- Agent dispatch.

## Requirements

- **R-001 (MUST):** The verifier must execute against candidate work.

## Acceptance criteria

### AC-001 — Candidate verified

**Verification**

- **Type:** gate
- **Check:** `CHK-001`

## Fixed decisions

- **D-001:** Use a deterministic executor fixture.

## Checklist

<!-- checklist:start -->
- [ ] **1** Verify.
    - [ ] **1.1** Verify candidate work. (`R-001`, `AC-001`, `CHK-001`)
<!-- checklist:end -->
"#;

const VERIFY: &str = r#"schema = "verify/v2"
task_id = "TASK-900"
writable_paths = ["src/**"]

[[checks]]
id = "CHK-001"
phase = "gate"
shell = "test -f src/verified.txt && printf VERIFIED_CANDIDATE"
requirements = ["R-001"]
acceptance = ["AC-001"]
expected = { stdout_contains = ["VERIFIED_CANDIDATE"] }
"#;

const CONFIG: &str = r#"schema = "experiment/v1"
[agents.default]
profile = "test"
[agents.profiles.test]
kind = "codex"
model = "test"
effort = "low"
image = "unused"
"#;

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// Child-process entry point selected by the fixture executable, never by browser input.
#[test]
fn executor_fixture() {
    let Some(config) = std::env::var_os("TASK_MONITOR_FIXTURE_CONFIG") else {
        return;
    };
    let package = PathBuf::from(std::env::var_os("TASK_MONITOR_FIXTURE_PACKAGE").unwrap());
    let resolved = crate::config::ExperimentConfig::load_resolved(Path::new(&config)).unwrap();
    let run_dir = resolved.runs_dir().join("fixture-run");
    let workspace = run_dir.join("workspace");
    let snapshot = run_dir.join("task-snapshot");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&snapshot).unwrap();
    for name in ["README.md", "verify.toml"] {
        std::fs::copy(package.join(name), snapshot.join(name)).unwrap();
    }
    let generated = crate::progress::generate(&snapshot).unwrap();
    let progress = generated
        .body
        .replace(
            "state: IN_PROGRESS\ncurrent: 1.1\nlatest_event: 1",
            "state: DONE\ncurrent: NONE\nlatest_event: 2",
        )
        .replace(
            "- 1 | STARTED | 1.1",
            "- 1 | STARTED | 1.1\n- 2 | DONE | 1.1",
        );
    std::fs::create_dir(run_dir.join("progress")).unwrap();
    std::fs::write(run_dir.join("progress/progress.md"), progress).unwrap();
    git(&workspace, &["init", "-q", "-b", "main"]);
    git(&workspace, &["config", "user.name", "Monitor fixture"]);
    git(
        &workspace,
        &["config", "user.email", "monitor@test.invalid"],
    );
    std::fs::write(workspace.join("base.txt"), "base\n").unwrap();
    git(&workspace, &["add", "base.txt"]);
    git(&workspace, &["commit", "-qm", "base"]);
    let base = git(&workspace, &["rev-parse", "HEAD"]);
    let started = chrono::Utc::now().timestamp_millis();
    std::fs::write(run_dir.join("fixture-started"), started.to_string()).unwrap();
    // Hold ownership long enough to observe conflict rejection and overlapping branches.
    let delay = std::fs::read_to_string(package.join("fixture-delay-ms"))
        .map(|value| value.parse::<u64>().unwrap())
        .unwrap_or(300);
    std::thread::sleep(Duration::from_millis(delay));
    std::fs::create_dir(workspace.join("src")).unwrap();
    let failing = package.join("fixture-fail").exists();
    std::fs::write(
        workspace.join(if failing {
            "src/wrong.txt"
        } else {
            "src/verified.txt"
        }),
        "candidate\n",
    )
    .unwrap();
    let mut manifest = Manifest {
        run: "fixture-run".into(),
        run_dir: run_dir.display().to_string(),
        container: "hermetic-no-container".into(),
        agent: "test".into(),
        agent_kind: "codex".into(),
        model: "test".into(),
        effort: "low".into(),
        task: "TASK-900".into(),
        repo_url: "file:///unused-local-fixture".into(),
        base_sha: base.clone(),
        clone_sha: base,
        lifecycle_predecessor_sha: None,
        session_id: "fixture".into(),
        pane: "none".into(),
        agent_name: "task".into(),
        start: crate::config::timestamp_rfc3339(),
        selfcheck: SELFCHECK_PASS.into(),
        experiment: None,
        gate: None,
        status_state: "GOAL_MET".into(),
        result_sha: None,
        pending_promotion_sha: None,
    };
    manifest.save(&run_dir).unwrap();
    let passed = crate::cmds::gate::gate_run(&run_dir, &resolved, &mut manifest).unwrap();
    assert_eq!(
        passed,
        !failing,
        "{}",
        std::fs::read_to_string(run_dir.join("out/gate.log")).unwrap()
    );
    std::fs::write(
        run_dir.join("fixture-finished"),
        chrono::Utc::now().timestamp_millis().to_string(),
    )
    .unwrap();
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

async fn setup(failing: bool, concurrency: usize) -> (tempfile::TempDir, AppState) {
    let root = tempfile::tempdir().unwrap();
    let group = root.path().join("tasks/project/group");
    std::fs::create_dir_all(&group).unwrap();
    std::fs::write(
        root.path().join("tasks/project/README.md"),
        "# Project\n\nIntegration fixture",
    )
    .unwrap();
    std::fs::write(group.join("README.md"), "# Group\n\nDependency diamond").unwrap();
    for (number, dependencies) in [
        ("001", vec![]),
        ("002", vec!["project/group/001"]),
        ("003", vec!["project/group/001"]),
        ("004", vec!["project/group/002", "project/group/003"]),
    ] {
        let task = group.join(number);
        std::fs::create_dir(&task).unwrap();
        std::fs::write(task.join("README.md"), README).unwrap();
        std::fs::write(task.join("verify.toml"), VERIFY).unwrap();
        std::fs::write(
            task.join("task.toml"),
            format!("schema='task-meta/v1'\nstatus='pending'\ndependencies={dependencies:?}\n"),
        )
        .unwrap();
        if failing && number == "001" {
            std::fs::write(task.join("fixture-fail"), "").unwrap();
        }
    }
    let executable = root.path().join("fixture-taskfmt");
    let test_binary = std::env::current_exe().unwrap();
    std::fs::write(&executable, format!("#!/bin/sh\nset -eu\nexport TASK_MONITOR_FIXTURE_CONFIG=\"$2\"\nexport TASK_MONITOR_FIXTURE_PACKAGE=\"$6\"\nexec {} --exact server::integration_tests::executor_fixture --nocapture --test-threads=1\n", shell_quote(test_binary.to_str().unwrap()))).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let experiment = root.path().join("experiment.toml");
    std::fs::write(&experiment, CONFIG).unwrap();
    let state = AppState::open(ServerConfig {
        tasks_root: root.path().join("tasks"),
        runs_root: root.path().join("runs"),
        adapter: Some(AdapterConfig {
            taskfmt: executable,
            experiment_config: experiment,
            repo: "file:///unused-local-fixture".into(),
            agent: None,
        }),
        concurrency,
        origin: "http://localhost:3000".into(),
    })
    .await
    .unwrap();
    (root, state)
}

async fn request(state: &AppState, method: &str, uri: &str) -> (StatusCode, Value) {
    let response = router(state.clone())
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("x-task-monitor", "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let json =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    (status, json)
}

async fn finished(state: &AppState) -> Value {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let (_, executions) = request(state, "GET", "/api/executions").await;
            if executions[0]["state"] != "running" {
                return executions[0].clone();
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await
    .expect("tracked execution must terminate")
}

#[tokio::test]
async fn project_http_run_uses_real_gate_and_respects_diamond_order_and_parallel_limit() {
    let (root, state) = setup(false, 2).await;
    let (status, _) = request(&state, "POST", "/api/run/project/project").await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let (status, _) = request(&state, "POST", "/api/run/task/project/group/001").await;
    assert_eq!(status, StatusCode::CONFLICT);
    let execution = finished(&state).await;
    assert_eq!(execution["state"], "done", "{execution}");
    let (status, catalog) = request(&state, "GET", "/api/catalog").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(catalog["summary"]["done"], 4);
    assert_eq!(catalog["summary"]["progress"], 100.0);
    let mut intervals = Vec::new();
    for number in ["001", "002", "003", "004"] {
        let id = format!("project/group/{number}");
        let artifact = execution["tasks"][&id]["artifact_id"].as_str().unwrap();
        let run = root.path().join("runs").join(artifact).join("fixture-run");
        let manifest = Manifest::load(&run).unwrap();
        assert!(super::adapter::verified(&manifest));
        let evidence: Value =
            serde_json::from_slice(&std::fs::read(run.join("out/gate-evidence.json")).unwrap())
                .unwrap();
        assert!(
            evidence["checks"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["name"] == "CHK-001" && c["evidence"]["stdout"] == "VERIFIED_CANDIDATE")
        );
        let start: i64 = std::fs::read_to_string(run.join("fixture-started"))
            .unwrap()
            .parse()
            .unwrap();
        let end: i64 = std::fs::read_to_string(run.join("fixture-finished"))
            .unwrap()
            .parse()
            .unwrap();
        intervals.push((start, end));
    }
    assert!(intervals[0].1 <= intervals[1].0 && intervals[0].1 <= intervals[2].0);
    assert!(intervals[1].1 <= intervals[3].0 && intervals[2].1 <= intervals[3].0);
    assert!(
        intervals[1].0 < intervals[2].1 && intervals[2].0 < intervals[1].1,
        "independent branches must overlap: {intervals:?}"
    );
    for (time, _) in &intervals {
        assert!(
            intervals
                .iter()
                .filter(|(s, e)| s <= time && time < e)
                .count()
                <= 2
        );
    }
    state.shutdown().await;
}

#[tokio::test]
async fn failed_real_gate_keeps_task_pending_and_never_launches_dependents() {
    let (_root, state) = setup(true, 2).await;
    assert_eq!(
        request(&state, "POST", "/api/run/group/project/group")
            .await
            .0,
        StatusCode::ACCEPTED
    );
    let execution = finished(&state).await;
    assert_eq!(execution["state"], "failed");
    assert_eq!(execution["tasks"].as_object().unwrap().len(), 1);
    assert_eq!(execution["tasks"]["project/group/001"]["state"], "failed");
    let (_, catalog) = request(&state, "GET", "/api/catalog").await;
    assert_eq!(catalog["summary"]["done"], 0);
    assert_eq!(
        catalog["tasks"]["project/group/001"]["metadata"]["status"],
        "pending"
    );
    assert_eq!(
        catalog["tasks"]["project/group/002"]["readiness"]["allowed"],
        false
    );
    state.shutdown().await;
}

#[tokio::test]
async fn concurrency_one_serializes_independent_ready_branches() {
    let (root, state) = setup(false, 1).await;
    assert_eq!(
        request(&state, "POST", "/api/run/project/project").await.0,
        StatusCode::ACCEPTED
    );
    let execution = finished(&state).await;
    assert_eq!(execution["state"], "done", "{execution}");
    let mut intervals = Vec::new();
    for number in ["002", "003"] {
        let artifact = execution["tasks"][format!("project/group/{number}")]["artifact_id"]
            .as_str()
            .unwrap();
        let run = root.path().join("runs").join(artifact).join("fixture-run");
        let start: i64 = std::fs::read_to_string(run.join("fixture-started"))
            .unwrap()
            .parse()
            .unwrap();
        let end: i64 = std::fs::read_to_string(run.join("fixture-finished"))
            .unwrap()
            .parse()
            .unwrap();
        intervals.push((start, end));
    }
    assert!(
        intervals[0].1 <= intervals[1].0 || intervals[1].1 <= intervals[0].0,
        "concurrency one must forbid ready branch overlap: {intervals:?}"
    );
    state.shutdown().await;
}

#[tokio::test]
async fn task_http_run_executes_only_the_selected_task() {
    let (_root, state) = setup(false, 2).await;
    assert_eq!(
        request(&state, "POST", "/api/run/task/project/group/001")
            .await
            .0,
        StatusCode::ACCEPTED
    );
    let execution = finished(&state).await;
    assert_eq!(execution["state"], "done");
    assert_eq!(
        execution["task_ids"],
        serde_json::json!(["project/group/001"])
    );
    assert_eq!(execution["tasks"].as_object().unwrap().len(), 1);
    let (_, catalog) = request(&state, "GET", "/api/catalog").await;
    assert_eq!(
        catalog["tasks"]["project/group/001"]["metadata"]["status"],
        "done"
    );
    for number in ["002", "003", "004"] {
        assert_eq!(
            catalog["tasks"][format!("project/group/{number}")]["metadata"]["status"],
            "pending"
        );
    }
    state.shutdown().await;
}

#[tokio::test]
async fn restart_reconciles_authentic_success_and_failure_gate_crash_windows() {
    for failing in [false, true] {
        let (root, state) = setup(failing, 2).await;
        assert_eq!(
            request(&state, "POST", "/api/run/task/project/group/001")
                .await
                .0,
            StatusCode::ACCEPTED
        );
        let mut execution = finished(&state).await;
        assert_eq!(execution["state"], if failing { "failed" } else { "done" });
        let config = state.inner.lock().unwrap().config.clone();
        state.shutdown().await;
        drop(state);
        // Replay only the durable write window: real gate evidence already exists, while
        // metadata and journal still record ownership from immediately before completion.
        let metadata_path = root.path().join("tasks/project/group/001/task.toml");
        let metadata = std::fs::read_to_string(&metadata_path).unwrap();
        let metadata = metadata
            .replace("status = \"done\"", "status = \"in_progress\"")
            .replace("status = \"pending\"", "status = \"in_progress\"");
        std::fs::write(metadata_path, metadata).unwrap();
        execution["state"] = "running".into();
        execution["finished"] = Value::Null;
        execution["tasks"]["project/group/001"]["state"] = "running".into();
        let journal = config
            .runs_root
            .join(format!("{}.json", execution["id"].as_str().unwrap()));
        std::fs::write(journal, serde_json::to_vec_pretty(&execution).unwrap()).unwrap();
        let recovered = AppState::open(config).await.unwrap();
        let (status, catalog) = request(&recovered, "GET", "/api/catalog").await;
        assert_eq!(status, StatusCode::OK);
        let task = &catalog["tasks"]["project/group/001"];
        assert_eq!(
            task["metadata"]["status"],
            if failing { "pending" } else { "done" }
        );
        if !failing {
            assert_eq!(task["latest_run"]["state"], "done");
        } else {
            assert_ne!(task["latest_run"]["state"], "done");
        }
        recovered.shutdown().await;
    }
}

#[tokio::test]
async fn fatal_catalog_error_drains_active_executor_and_halts_new_execution() {
    let (root, state) = setup(false, 2).await;
    std::fs::write(
        root.path().join("tasks/project/group/003/fixture-delay-ms"),
        "10000",
    )
    .unwrap();
    // Keep the other branch alive long enough for deterministic corruption after both start.
    std::fs::write(
        root.path().join("tasks/project/group/002/fixture-delay-ms"),
        "1000",
    )
    .unwrap();
    assert_eq!(
        request(&state, "POST", "/api/run/project/project").await.0,
        StatusCode::ACCEPTED
    );
    let active_artifact = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let (_, executions) = request(&state, "GET", "/api/executions").await;
            let tasks = &executions[0]["tasks"];
            if let (Some(left), Some(right)) = (
                tasks["project/group/002"]["artifact_id"].as_str(),
                tasks["project/group/003"]["artifact_id"].as_str(),
            ) {
                let left = root
                    .path()
                    .join("runs")
                    .join(left)
                    .join("fixture-run/fixture-started");
                let right_root = root.path().join("runs").join(right);
                if left.exists() && right_root.join("fixture-run/fixture-started").exists() {
                    break right_root;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let metadata_path = root.path().join("tasks/project/group/004/task.toml");
    let original = std::fs::read_to_string(&metadata_path).unwrap();
    std::fs::write(&metadata_path, "invalid catalog metadata").unwrap();
    let execution = finished(&state).await;
    assert_eq!(execution["state"], "failed");
    assert_eq!(
        execution["tasks"]["project/group/003"]["state"],
        "interrupted"
    );
    assert!(
        !active_artifact
            .join("fixture-run/fixture-finished")
            .exists(),
        "long running executor must be cancelled before candidate completion"
    );
    let _released_supervisor = crate::monitor::RunLock::acquire(&active_artifact)
        .expect("terminal scheduler must have joined child cleanup and released its lease");
    std::fs::write(metadata_path, original).unwrap();
    let (status, catalog) = request(&state, "GET", "/api/catalog").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(catalog["execution"]["available"], false);
    assert_eq!(
        request(&state, "POST", "/api/run/task/project/group/004")
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    state.shutdown().await;
}
