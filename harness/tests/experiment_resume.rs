//! Repo resolution for `taskfmt experiment`: a resume is pinned to the experiment's recorded
//! `repo_url` and must never create a repo, while a fresh experiment still mints one.

use taskfmt::cmds::Ctx;
use taskfmt::cmds::experiment::{resolve_repo_url, resume_repo_url};
use taskfmt::config::{ExperimentConfig, Resolved};
use taskfmt::interactive::Interaction;
use taskfmt::runstate::{ExperimentState, ExperimentTask, RepoRecord};

const MANIFEST: &str = r#"
schema = "experiment/v1"
[github]
owner = "taskfmt-tests-no-such-owner"
repo_prefix = "taskfmt-experiment"
[paths]
tasks_dir = "tasks"
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

const REPO: &str = "https://github.com/donbeave/taskfmt-experiment-20260828-164451";
const OTHER: &str = "https://github.com/donbeave/taskfmt-experiment-20260828-175830";
/// Never clonable and never a real host: used where a test needs dispatch to fail at the clone.
const UNREACHABLE: &str = "https://github.invalid/taskfmt-tests/no-such-repo.git";

/// A config rooted at a temp dir, with one recorded experiment and its task dirs.
struct Fixture {
    _dir: tempfile::TempDir,
    resolved: Resolved,
    ctx: Ctx,
    experiment_id: String,
}

fn state(repo_url: &str, tasks: &[(&str, &str, bool)]) -> ExperimentState {
    let mut state = ExperimentState::new("exp-20260828-165259", repo_url);
    for (task, gate, pushed) in tasks {
        state.tasks.push(ExperimentTask {
            task: (*task).to_string(),
            repo_url: repo_url.to_string(),
            base_sha: "base".into(),
            result_sha: None,
            gate: (*gate).to_string(),
            pushed: *pushed,
            run_dir: format!("/tmp/{task}"),
        });
    }
    state
}

fn fixture(tasks: &[(&str, &str, bool)]) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("experiment.toml"), MANIFEST).unwrap();
    let (cfg, root) = ExperimentConfig::load(&root.join("experiment.toml")).unwrap();
    let resolved = Resolved::new(&root, cfg);
    for name in ["TASK-001", "TASK-002"] {
        let task = resolved.tasks_dir().join(name);
        std::fs::create_dir_all(&task).unwrap();
        std::fs::write(task.join("README.md"), "x").unwrap();
    }
    let experiment_id = "exp-20260828-165259".to_string();
    state(REPO, tasks)
        .save(&resolved.experiment_file(&experiment_id))
        .unwrap();
    let ctx = Ctx {
        config_path: dir.path().join("experiment.toml"),
        verbose: false,
        // auto-yes: any repo creation attempt would run `gh` against the bogus owner above
        interaction: Interaction::new(true, true),
    };
    Fixture {
        _dir: dir,
        resolved,
        ctx,
        experiment_id,
    }
}

/// A `create` fallback that fails the test if it is ever reached.
fn no_create() -> impl FnOnce(Option<&str>) -> anyhow::Result<String> {
    |_| panic!("a recorded experiment must never create a repo")
}

/// A `create` fallback that records the argument it was handed (`None` means "mint a new repo").
struct SpyCreate {
    seen: std::rc::Rc<std::cell::RefCell<Option<String>>>,
}

impl SpyCreate {
    fn new() -> Self {
        Self {
            seen: std::rc::Rc::new(std::cell::RefCell::new(None)),
        }
    }

    fn fallback(&self) -> impl FnOnce(Option<&str>) -> anyhow::Result<String> + '_ {
        let seen = self.seen.clone();
        move |provided: Option<&str>| {
            *seen.borrow_mut() = provided.map(str::to_string);
            Ok(match provided {
                Some(url) => url.to_string(),
                None => "https://github.com/owner/created".to_string(),
            })
        }
    }

    fn called_with(&self) -> Option<String> {
        self.seen.borrow().clone()
    }
}

#[test]
fn a_resume_without_repo_arg_uses_the_recorded_repo() {
    let recorded = state(REPO, &[("TASK-001", "pass", true)]);
    assert_eq!(resume_repo_url(&recorded, None).unwrap(), REPO);
}

#[test]
fn a_resume_accepts_a_repo_arg_that_matches_the_record() {
    let recorded = state(REPO, &[("TASK-001", "pass", true)]);
    assert_eq!(resume_repo_url(&recorded, Some(REPO)).unwrap(), REPO);
}

#[test]
fn a_resume_refuses_a_conflicting_repo_arg() {
    let recorded = state(REPO, &[("TASK-001", "pass", true)]);
    let err = resume_repo_url(&recorded, Some(OTHER))
        .unwrap_err()
        .to_string();
    assert!(err.contains(OTHER), "{err}");
    assert!(err.contains(REPO), "{err}");
    assert!(err.contains("recorded repo"), "{err}");
}

#[test]
fn recorded_state_pins_the_repo_and_skips_creation() {
    let recorded = state(REPO, &[("TASK-001", "pass", true)]);
    let got = resolve_repo_url(Some(&recorded), None, no_create()).unwrap();
    assert_eq!(got, REPO);
}

#[test]
fn no_state_and_no_repo_arg_goes_through_the_create_fallback() {
    let spy = SpyCreate::new();
    let got = resolve_repo_url(None, None, spy.fallback()).unwrap();
    assert_eq!(got, "https://github.com/owner/created");
    assert_eq!(spy.called_with(), None, "create is asked for a new repo");
}

#[test]
fn no_state_with_a_repo_arg_hands_it_to_the_create_fallback() {
    // repo::ensure_repo keeps its "provided wins" behaviour, so the fallback sees the argument
    let spy = SpyCreate::new();
    let got = resolve_repo_url(None, Some(REPO), spy.fallback()).unwrap();
    assert_eq!(got, REPO);
    assert_eq!(spy.called_with(), Some(REPO.to_string()));
}

/// The production bug: `experiment --auto --resume <id>` without `--repo` minted a fresh repo
/// instead of continuing on the recorded one. Here it must finish without touching `gh` at all —
/// every selected task is already pushed, and the fixture's GitHub owner does not exist, so any
/// creation attempt would fail the test.
#[test]
fn resuming_a_finished_experiment_creates_no_repo() {
    let fx = fixture(&[("TASK-001", "pass", true), ("TASK-002", "pass", true)]);
    let code = taskfmt::cmds::experiment::run(
        &fx.ctx,
        &[String::from("all")],
        None,
        None,
        Some(&fx.experiment_id),
        false,
    )
    .unwrap();
    assert_eq!(code, 0, "nothing left to run");
    let repos = RepoRecord::load_all(&fx.resolved.runs_dir()).unwrap();
    assert!(repos.is_empty(), "no repo was created: {repos:?}");
}

/// Same bug through the conflicting-`--repo` path: the error names both URLs and stops before any
/// dispatch.
#[test]
fn resuming_with_a_conflicting_repo_arg_fails_before_dispatch() {
    let fx = fixture(&[("TASK-001", "pass", true)]);
    let err = taskfmt::cmds::experiment::run(
        &fx.ctx,
        &[String::from("all")],
        Some(OTHER),
        None,
        Some(&fx.experiment_id),
        false,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains(OTHER), "{err}");
    assert!(err.contains(REPO), "{err}");
}

/// Resuming an id with no state must say so up front, not mint a repo first (the old order created
/// one before discovering there was nothing to resume).
#[test]
fn resuming_a_missing_experiment_says_so_without_creating_a_repo() {
    let fx = fixture(&[]);
    let err = taskfmt::cmds::experiment::run(
        &fx.ctx,
        &[String::from("all")],
        None,
        None,
        Some("exp-nope"),
        false,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("nothing to resume"), "{err}");
    assert!(!err.contains("gh repo create"), "{err}");
    let repos = RepoRecord::load_all(&fx.resolved.runs_dir()).unwrap();
    assert!(repos.is_empty(), "no repo was created: {repos:?}");
}

/// Same rule for `taskfmt run --exp <id>`: the recorded repo wins and the create fallback is never
/// reached. The recorded URL is a path that does not exist, so dispatch fails at the clone — the
/// error naming that path is the proof that the recorded repo was chosen.
#[test]
fn run_with_exp_tag_uses_the_recorded_repo() {
    let fx = fixture(&[]);
    let recorded = fx.resolved.root.join("no-such-repo.git");
    let recorded = recorded.display().to_string();
    state(&recorded, &[])
        .save(&fx.resolved.experiment_file(&fx.experiment_id))
        .unwrap();
    let err = taskfmt::cmds::run::run(
        &fx.ctx,
        "TASK-001",
        None,
        None,
        None,
        None,
        false,
        None,
        Some(&fx.experiment_id),
        false,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains(&recorded), "{err}");
    assert!(err.contains("cloning"), "{err}");
    assert!(!err.contains("gh repo create"), "{err}");
    let repos = RepoRecord::load_all(&fx.resolved.runs_dir()).unwrap();
    assert!(repos.is_empty(), "no repo was created: {repos:?}");
}

/// `taskfmt run --exp <id> --repo <other>` refuses before any dispatch, exactly like a resume.
#[test]
fn run_with_exp_tag_refuses_a_conflicting_repo_arg() {
    let fx = fixture(&[]);
    let err = taskfmt::cmds::run::run(
        &fx.ctx,
        "TASK-001",
        Some(OTHER),
        None,
        None,
        None,
        false,
        None,
        Some(&fx.experiment_id),
        false,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains(OTHER), "{err}");
    assert!(err.contains(REPO), "{err}");
    assert!(!err.contains("cloning"), "{err}");
    let repos = RepoRecord::load_all(&fx.resolved.runs_dir()).unwrap();
    assert!(repos.is_empty(), "no repo was created: {repos:?}");
}

/// Without recorded state the old behaviour stands: `--repo` is used as-is (the create-only path is
/// covered by the fallback tests above). The argument must be unclonable so dispatch fails at the
/// clone instead of reaching a real repository.
#[test]
fn run_without_exp_state_honours_the_repo_arg() {
    let fx = fixture(&[]);
    let err = taskfmt::cmds::run::run(
        &fx.ctx,
        "TASK-001",
        Some(UNREACHABLE),
        None,
        None,
        None,
        false,
        None,
        Some("exp-fresh"),
        false,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains(UNREACHABLE), "{err}");
    assert!(err.contains("cloning"), "{err}");
    let repos = RepoRecord::load_all(&fx.resolved.runs_dir()).unwrap();
    assert!(repos.is_empty(), "no repo was created: {repos:?}");
}
