//! Per-task execution.toml resolution without Docker.

use std::path::{Path, PathBuf};

use taskfmt::cmds::run::plan_dispatch_for_task;
use taskfmt::config::{DispatchOverrides, ExperimentConfig, Resolved, resolve_dispatch};
use taskfmt::executioncfg::ExecutionConfig;

fn example_task() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/example")
}

const MANIFEST: &str = r#"
schema = "experiment/v1"
[paths]
tasks_dir = "tasks"
runs_dir = "runs"
[agents.default]
profile = "default-p"
[agents.profiles.default-p]
kind = "claude"
model = "default-model"
effort = "low"
image = "harness-claude:latest"
[agents.profiles.alt-p]
kind = "codex"
model = "alt-model"
effort = "max"
image = "harness-codex:latest"
"#;

struct Fixture {
    _dir: tempfile::TempDir,
    resolved: Resolved,
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in walkdir::WalkDir::new(from).into_iter().flatten() {
        let rel = entry.path().strip_prefix(from).unwrap();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = to.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn fixture_with_execution(execution: Option<&str>) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("experiment.toml"), MANIFEST).unwrap();
    let task = root.join("tasks").join("TASK-042");
    copy_tree(&example_task(), &task);
    if let Some(text) = execution {
        std::fs::write(task.join("execution.toml"), text).unwrap();
    }
    let (cfg, root) = ExperimentConfig::load(&root.join("experiment.toml")).unwrap();
    Fixture {
        _dir: dir,
        resolved: Resolved::new(&root, cfg),
    }
}

#[test]
fn without_execution_toml_uses_experiment_default() {
    let fx = fixture_with_execution(None);
    let (profile, model, effort) =
        plan_dispatch_for_task(&fx.resolved, "TASK-042", None, None, None).unwrap();
    assert_eq!(profile, "default-p");
    assert_eq!(model, "default-model");
    assert_eq!(effort, "low");
}

#[test]
fn execution_toml_selects_profile_model_and_effort() {
    let fx = fixture_with_execution(Some(
        r#"
schema = "execution/v1"
profile = "alt-p"
model = "exec-model"
effort = "high"
"#,
    ));
    let (profile, model, effort) =
        plan_dispatch_for_task(&fx.resolved, "TASK-042", None, None, None).unwrap();
    assert_eq!(profile, "alt-p");
    assert_eq!(model, "exec-model");
    assert_eq!(effort, "high");
}

#[test]
fn cli_agent_overrides_execution_profile() {
    let fx = fixture_with_execution(Some(
        r#"
schema = "execution/v1"
profile = "alt-p"
model = "exec-model"
effort = "high"
"#,
    ));
    let (profile, model, effort) = plan_dispatch_for_task(
        &fx.resolved,
        "TASK-042",
        Some("default-p"),
        None,
        None,
    )
    .unwrap();
    assert_eq!(profile, "default-p");
    assert_eq!(model, "exec-model");
    assert_eq!(effort, "high");
}

#[test]
fn cli_model_and_effort_override_execution_and_profile() {
    let fx = fixture_with_execution(Some(
        r#"
schema = "execution/v1"
profile = "alt-p"
"#,
    ));
    let (profile, model, effort) = plan_dispatch_for_task(
        &fx.resolved,
        "TASK-042",
        None,
        Some("cli-model"),
        Some("medium"),
    )
    .unwrap();
    assert_eq!(profile, "alt-p");
    assert_eq!(model, "cli-model");
    assert_eq!(effort, "medium");
}

#[test]
fn resolve_dispatch_applies_model_to_profile_clone() {
    let cfg = ExperimentConfig::parse(MANIFEST).unwrap();
    let exec = ExecutionConfig::parse(
        r#"
schema = "execution/v1"
profile = "alt-p"
model = "resolved-model"
"#,
    )
    .unwrap();
    let got = resolve_dispatch(
        &cfg,
        Some(&exec),
        DispatchOverrides {
            agent: None,
            model: None,
            effort: Some("low"),
        },
    )
    .unwrap();
    assert_eq!(got.profile.model, "resolved-model");
    assert_eq!(got.profile.effort, "low");
    assert_eq!(got.model, "resolved-model");
    assert_eq!(got.effort, "low");
}
