//! Run-argument resolution: run id, container name, manifest container, and the error hint.

use std::path::{Path, PathBuf};

use taskfmt::cmds::Ctx;
use taskfmt::cmds::{RECENT_RUN_HINTS, resolve_run_arg};
use taskfmt::config::{ExperimentConfig, Resolved};
use taskfmt::interactive::Interaction;
use taskfmt::runstate::{GateRecord, Manifest};

const MANIFEST: &str = r#"
schema = "experiment/v1"
[paths]
runs_dir = "runs"
[agents.default]
profile = "zai-flash"
[agents.profiles.zai-flash]
kind = "claude"
model = "glm-5.3-flash"
effort = "low"
image = "harness-claude:latest"
[agents.profiles.zai-flash.env_secret]
ANTHROPIC_AUTH_TOKEN = "op://vault/item/section/field"
"#;

/// A config rooted at a temp dir: `runs_dir = <root>/runs`.
struct Fixture {
    _dir: tempfile::TempDir,
    resolved: Resolved,
    ctx: Ctx,
}

fn manifest(run: &str, container: &str) -> Manifest {
    Manifest {
        run: run.to_string(),
        run_dir: format!("/tmp/{run}"),
        container: container.to_string(),
        agent: "zai-flash".into(),
        agent_kind: "claude".into(),
        model: "glm-5.3-flash".into(),
        effort: "low".into(),
        task: "TASK-001".into(),
        repo_url: "https://github.com/donbeave/x.git".into(),
        base_sha: "abc".into(),
        clone_sha: "def".into(),
        session_id: "00000000-0000-4000-8000-000000000000".into(),
        pane: "pane-1".into(),
        agent_name: "task".into(),
        start: "2026-08-28T10:00:00Z".into(),
        experiment: None,
        selfcheck: taskfmt::runstate::SELFCHECK_NOT_RUN.into(),
        gate: Some(GateRecord {
            verdict: "fail".into(),
            exit: 1,
            last_line: "RESULT FAIL".into(),
            head: "head".into(),
            log: "/tmp/gate.log".into(),
            finished: "2026-08-28T11:00:00Z".into(),
        }),
        result_sha: None,
    }
}

/// `n` runs, newest last: `20260828-0000<i>-zai-flash-TASK-001`.
fn fixture_with(runs: &[(&str, &str)]) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("experiment.toml"), MANIFEST).unwrap();
    let (cfg, root) = ExperimentConfig::load(&root.join("experiment.toml")).unwrap();
    let resolved = Resolved::new(&root, cfg);
    for (id, container) in runs {
        let run_dir = resolved.run_dir(id);
        std::fs::create_dir_all(&run_dir).unwrap();
        manifest(id, container).save(&run_dir).unwrap();
    }
    let ctx = Ctx {
        config_path: dir.path().join("experiment.toml"),
        verbose: false,
        interaction: Interaction::new(true, true),
    };
    Fixture {
        _dir: dir,
        resolved,
        ctx,
    }
}

fn newest(n: usize) -> Vec<(String, String)> {
    (0..n)
        .map(|i| {
            (
                format!("20260828-0000{i:02}-zai-flash-TASK-001"),
                format!("harness-20260828-0000{i:02}-zai-flash-TASK-001"),
            )
        })
        .collect()
}

#[test]
fn exact_run_id_resolves() {
    let fx = fixture_with(&[(
        "20260828-000000-zai-flash-TASK-001",
        "harness-20260828-000000-zai-flash-TASK-001",
    )]);
    let got = resolve_run_arg(&fx.resolved, "20260828-000000-zai-flash-TASK-001").unwrap();
    assert_eq!(
        got,
        fx.resolved.run_dir("20260828-000000-zai-flash-TASK-001")
    );
}

#[test]
fn container_name_resolves_to_the_run_dir() {
    let fx = fixture_with(&[(
        "20260828-000000-zai-flash-TASK-001",
        "harness-20260828-000000-zai-flash-TASK-001",
    )]);
    let got = resolve_run_arg(&fx.resolved, "harness-20260828-000000-zai-flash-TASK-001").unwrap();
    assert_eq!(
        got,
        fx.resolved.run_dir("20260828-000000-zai-flash-TASK-001")
    );
}

#[test]
fn container_name_without_a_manifest_still_strips_to_the_run_dir() {
    let fx = fixture_with(&[]);
    let run_dir = fx.resolved.run_dir("20260828-000000-zai-flash-TASK-001");
    std::fs::create_dir_all(&run_dir).unwrap();
    // no manifest.json: the id is unambiguous on its own
    let got = resolve_run_arg(&fx.resolved, "harness-20260828-000000-zai-flash-TASK-001").unwrap();
    assert_eq!(got, run_dir);
}

#[test]
fn a_manifest_container_that_is_not_the_run_id_resolves_by_scan() {
    // a future container-naming change must not break the argument
    let fx = fixture_with(&[("20260828-000000-zai-flash-TASK-001", "taskfmt-run-42")]);
    assert_eq!(
        resolve_run_arg(&fx.resolved, "taskfmt-run-42").unwrap(),
        fx.resolved.run_dir("20260828-000000-zai-flash-TASK-001")
    );
    // and the prefixed form of that id is not a container name
    let err = resolve_run_arg(&fx.resolved, "harness-20260828-000000-zai-flash-TASK-001")
        .unwrap_err()
        .to_string();
    assert!(err.contains("no such run"), "{err}");
}

#[test]
fn a_container_name_whose_manifest_disagrees_is_rejected() {
    let fx = fixture_with(&[
        (
            "20260828-000001-zai-flash-TASK-002",
            "harness-20260828-000001-zai-flash-TASK-002",
        ),
        (
            "20260828-000000-zai-flash-TASK-001",
            "harness-20260828-000000-zai-flash-TASK-001",
        ),
    ]);
    let err = resolve_run_arg(&fx.resolved, "harness-not-this-run")
        .unwrap_err()
        .to_string();
    assert!(err.contains("no such run"), "{err}");
}

#[test]
fn unknown_argument_names_the_path_and_hints_recent_runs() {
    let runs = newest(8);
    let refs: Vec<(&str, &str)> = runs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let fx = fixture_with(&refs);
    let arg = "harness-does-not-exist";
    let err = resolve_run_arg(&fx.resolved, arg).unwrap_err().to_string();
    assert!(
        err.starts_with(&format!(
            "no such run: {}",
            fx.resolved.run_dir(arg).display()
        )),
        "{err}"
    );
    assert!(err.contains("container name"), "{err}");
    // newest first, capped
    assert!(err.contains("20260828-000007-zai-flash-TASK-001"), "{err}");
    assert!(!err.contains("20260828-000002-zai-flash-TASK-001"), "{err}");
    let listed = err.split("recent runs: ").nth(1).unwrap();
    assert_eq!(listed.split(", ").count(), RECENT_RUN_HINTS, "{err}");
    assert!(
        listed.starts_with("20260828-000007-zai-flash-TASK-001"),
        "{err}"
    );
}

#[test]
fn unknown_argument_without_any_runs_says_so() {
    let fx = fixture_with(&[]);
    let err = resolve_run_arg(&fx.resolved, "ghost")
        .unwrap_err()
        .to_string();
    assert!(err.contains("recent runs: (none)"), "{err}");
}

#[test]
fn recent_runs_skips_dirs_without_a_manifest() {
    let fx = fixture_with(&[(
        "20260828-000000-zai-flash-TASK-001",
        "harness-20260828-000000-zai-flash-TASK-001",
    )]);
    let runs_dir = fx.resolved.runs_dir();
    for noise in ["exp-20260828-000000", "repos"] {
        std::fs::create_dir_all(runs_dir.join(noise)).unwrap();
    }
    let got = taskfmt::cmds::recent_runs(&runs_dir);
    let ids: Vec<String> = got.iter().map(|(_, id)| id.clone()).collect();
    assert_eq!(ids, vec!["20260828-000000-zai-flash-TASK-001".to_string()]);
    assert_eq!(
        PathBuf::from(&got[0].0),
        runs_dir.join("20260828-000000-zai-flash-TASK-001")
    );
    assert!(taskfmt::cmds::recent_runs(Path::new("/nonexistent-runs")).is_empty());
}

/// `status`, `gate`, `promote` and `attach` must all route through the shared resolver: an unknown
/// argument reports the hint, not a bare path or a manifest read failure.
#[test]
fn every_run_command_routes_through_the_resolver() {
    let fx = fixture_with(&[(
        "20260828-000000-zai-flash-TASK-001",
        "harness-20260828-000000-zai-flash-TASK-001",
    )]);
    for err in [
        taskfmt::cmds::status::run(&fx.ctx, "ghost", false, None)
            .unwrap_err()
            .to_string(),
        taskfmt::cmds::gate::run(&fx.ctx, "ghost")
            .unwrap_err()
            .to_string(),
        taskfmt::cmds::promote::run(&fx.ctx, "ghost", true)
            .unwrap_err()
            .to_string(),
        taskfmt::cmds::attach::run(&fx.ctx, "ghost")
            .unwrap_err()
            .to_string(),
    ] {
        assert!(
            err.contains("recent runs: 20260828-000000-zai-flash-TASK-001"),
            "{err}"
        );
    }
}

/// The container-name form reaches the command body, not just the resolver.
#[test]
fn promote_accepts_the_container_name() {
    let fx = fixture_with(&[(
        "20260828-000000-zai-flash-TASK-001",
        "harness-20260828-000000-zai-flash-TASK-001",
    )]);
    let err =
        taskfmt::cmds::promote::run(&fx.ctx, "harness-20260828-000000-zai-flash-TASK-001", true)
            .unwrap_err()
            .to_string();
    // resolution succeeded; promote now refuses for the real reason (no workspace yet)
    let expected = fx
        .resolved
        .run_dir("20260828-000000-zai-flash-TASK-001")
        .display()
        .to_string();
    assert!(err.contains(&expected), "{err}");
}
