//! Command implementations. `dispatch` maps the parsed CLI to a module and a process exit code.

pub mod agent_launch;
pub mod attach;
pub mod build_images;
pub mod container_entrypoint;
pub mod experiment;
pub mod fingerprint;
pub mod gate;
pub mod lint;
pub mod preload;
pub mod progress_init;
pub mod promote;
pub mod ps;
pub mod repo;
pub mod run;
pub mod selfcheck;
pub mod selftest;
pub mod status;
pub mod verify;

use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, bail};

use crate::cli::Cli;
use crate::config::{ExperimentConfig, MANIFEST_NAME, Resolved, discover_upward};
use crate::interactive::Interaction;
use crate::ops::container::{self, CONTAINER_PREFIX};
use crate::ops::docker::ContainerInfo;
use crate::runstate::{MANIFEST_FILE, Manifest};

/// Everything a command needs besides its own arguments.
pub struct Ctx {
    pub config_path: PathBuf,
    pub verbose: bool,
    pub interaction: Interaction,
}

/// The manifest to read: `--config` > `$TASKFMT_CONFIG` > the repo-relative default.
///
/// An explicit choice is made absolute here, so `Ctx::load` reads an absolute `config_path` as
/// "the operator named this file" (original semantics: relative to the cwd at the time it was
/// given) and a relative one as "discover it by walking up from the cwd". `TASKFMT_CONFIG` joins
/// the `TASKFMT_ROOT` / `TASKFMT_TASK_DIR` / `TASKFMT_BASE` convention already used by `verify`,
/// and lets an operator pin one manifest for a whole session without a flag on every command.
fn config_path(explicit: Option<PathBuf>, env: Option<PathBuf>) -> PathBuf {
    match explicit.or(env) {
        Some(path) => std::path::absolute(&path).unwrap_or(path),
        None => PathBuf::from(crate::config::MANIFEST_NAME),
    }
}

impl Ctx {
    pub fn from_cli(cli: &Cli) -> Self {
        let env = std::env::var_os("TASKFMT_CONFIG")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Self {
            config_path: config_path(cli.config.clone(), env),
            verbose: cli.verbose,
            interaction: Interaction::new(cli.auto, cli.yes),
        }
    }

    /// Load the experiment manifest and the path resolver rooted at its directory. A relative
    /// `config_path` is discovered by walking up from the cwd (`ExperimentConfig::resolve_path`).
    pub fn load(&self) -> anyhow::Result<Resolved> {
        ExperimentConfig::load_resolved(&self.config_path)
    }
}

/// What a `<RUN>` argument pointed at, worked out before any manifest is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunLocation {
    /// The run's own directory on the host.
    pub run_dir: PathBuf,
    /// The manifest that dispatched the run, when the container names one (`taskfmt.manifest`).
    pub manifest: Option<PathBuf>,
}

/// The manifest **and** the run directory for a `<RUN>` argument — the pair every run command
/// needs, resolved once.
///
/// Decision order, and the reason for it: a run's identity belongs to the run, not to the
/// directory the operator is standing in. So the argument is resolved **against the run itself
/// first** — as a run directory, as the path of a `manifest.json`, or as a container name (with or
/// without the `harness-` prefix) whose `taskfmt.*` labels, or failing those whose `/work` bind
/// mount, say where the run lives and which `experiment.toml` dispatched it. Only when the argument
/// names no run on this host does `--config` / `$TASKFMT_CONFIG` / cwd discovery get a say. The old
/// order had it backwards: the manifest found next to the process decided what a run was, so the
/// same command answered differently from two different directories.
pub fn load_run(ctx: &Ctx, arg: &str) -> anyhow::Result<(Resolved, PathBuf)> {
    let location = locate_run_dir(arg);
    let run_err = match &location {
        Some(found) => match config_for_location(found) {
            Ok(resolved) => return Ok((resolved, found.run_dir.clone())),
            Err(err) => err,
        },
        None => not_located(arg),
    };
    match ctx.load() {
        Ok(resolved) => {
            let run_dir = match &location {
                Some(found) => found.run_dir.clone(),
                None => resolve_run_arg(&resolved, arg)?,
            };
            Ok((resolved, run_dir))
        }
        Err(cwd_err) => bail!(
            "cannot resolve a manifest for {arg:?} from the run itself: {run_err:#}; nor from \
             this process: {cwd_err:#}. Run `taskfmt ps` to list the runs on this host, or name \
             the manifest with --config <path> (or $TASKFMT_CONFIG)"
        ),
    }
}

/// [`load_run`]'s manifest half, for callers that need no run directory.
pub fn load_for_run(ctx: &Ctx, arg: &str) -> anyhow::Result<Resolved> {
    load_run(ctx, arg).map(|(resolved, _)| resolved)
}

/// The error when the argument names no run on this host.
fn not_located(arg: &str) -> anyhow::Error {
    anyhow!(
        "cannot locate a run directory for {arg:?}: `docker inspect` knows no container named \
         {CONTAINER_PREFIX}{arg} or {arg}, and {arg:?} is neither a directory holding \
         {MANIFEST_FILE} nor the path of one"
    )
}

/// The manifest for a located run: the `taskfmt.manifest` label when the container names one that
/// is still on disk (that manifest *is* the one this run was dispatched with), else the nearest
/// `experiment.toml` at or above the run directory.
fn config_for_location(location: &RunLocation) -> anyhow::Result<Resolved> {
    if let Some(manifest) = location.manifest.as_deref() {
        return ExperimentConfig::load_resolved(manifest);
    }
    let (found, searched) = discover_upward(&location.run_dir, Path::new(MANIFEST_NAME));
    let manifest_path = found.ok_or_else(|| {
        anyhow!(
            "found run dir {} but no {MANIFEST_NAME} in it or any ancestor; searched: {}",
            location.run_dir.display(),
            searched
                .iter()
                .map(|dir| dir.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    ExperimentConfig::load_resolved(&manifest_path)
}

/// Find a run from the `<RUN>` argument alone, with no manifest in hand.
fn locate_run_dir(arg: &str) -> Option<RunLocation> {
    locate_run_dir_with(arg, crate::ops::docker::inspect_container)
}

/// The cwd-free decision order behind [`locate_run_dir`], with `docker inspect` injected so the
/// order is unit-testable without a real docker:
///
/// 1. the argument as the path of a run's `manifest.json`, or of a directory holding one — an
///    explicit path is the operator naming the run outright, and costs no docker call;
/// 2. the argument as a container name, `harness-<arg>` first and then bare `<arg>`, so the
///    container name and the run id are interchangeable everywhere;
/// 3. nothing.
///
/// For (2) the container's `taskfmt.run_dir` / `taskfmt.manifest` labels are preferred, and the
/// parent of its `/work` bind mount is the fallback that keeps containers launched before those
/// labels existed locatable.
fn locate_run_dir_with(
    arg: &str,
    inspect: impl Fn(&str) -> Option<ContainerInfo>,
) -> Option<RunLocation> {
    let as_path = PathBuf::from(arg);
    if as_path.is_file() && as_path.file_name() == Some(std::ffi::OsStr::new(MANIFEST_FILE)) {
        let run_dir = as_path.parent().unwrap_or(Path::new("."));
        return Some(RunLocation {
            run_dir: std::fs::canonicalize(run_dir).unwrap_or_else(|_| run_dir.to_path_buf()),
            manifest: None,
        });
    }
    if as_path.is_dir() && as_path.join(MANIFEST_FILE).is_file() {
        return Some(RunLocation {
            run_dir: std::fs::canonicalize(&as_path).unwrap_or(as_path),
            manifest: None,
        });
    }
    let candidates = if arg.starts_with(CONTAINER_PREFIX) {
        vec![arg.to_string()]
    } else {
        vec![format!("{CONTAINER_PREFIX}{arg}"), arg.to_string()]
    };
    for name in candidates {
        if let Some(info) = inspect(&name)
            && let Some(run_dir) = container::run_dir_of(&info)
        {
            return Some(RunLocation {
                run_dir,
                manifest: container::manifest_of(&info),
            });
        }
    }
    None
}

/// Dispatch one parsed CLI. Returns the process exit code.
pub fn dispatch(cli: &Cli) -> anyhow::Result<i32> {
    let ctx = Ctx::from_cli(cli);
    use crate::cli::{Command, RepoCmd};
    match &cli.command {
        Command::Lint { tasks } => lint::run(&ctx, tasks),
        Command::ProgressInit { task, out } => progress_init::run(&ctx, task, out.as_deref()),
        Command::Selftest => selftest::run(&ctx),
        Command::Fingerprint { path, image } => fingerprint::run(path.as_deref(), image.as_deref()),
        Command::Verify {
            root,
            task_dir,
            progress,
            no_progress,
            base,
            log_dir,
            fail_fast,
        } => verify::run(
            root.as_deref(),
            task_dir.as_deref(),
            if *no_progress {
                Some(String::new())
            } else {
                progress.clone()
            },
            base.clone(),
            log_dir.as_deref(),
            *fail_fast,
        ),
        Command::Selfcheck {
            task,
            workspace,
            base,
            reference,
            keep,
        } => selfcheck::run(task, workspace, base.clone(), reference.as_deref(), *keep),
        Command::BuildImages { agent, no_cache } => {
            build_images::run(&ctx, (*agent).into(), *no_cache)
        }
        Command::Preload => preload::run(&ctx),
        Command::Repo { cmd } => match cmd {
            RepoCmd::Create { name } => repo::create(&ctx, name.as_deref()),
            RepoCmd::Delete { name, yes } => repo::delete(&ctx, name.as_deref(), *yes),
        },
        Command::Run {
            task,
            repo,
            agent,
            model,
            effort,
            wait,
            kill_after,
            exp,
            selfcheck,
        } => run::run(
            &ctx,
            task,
            repo.as_deref(),
            agent.as_deref(),
            model.as_deref(),
            effort.as_deref(),
            *wait,
            *kill_after,
            exp.as_deref(),
            *selfcheck,
            // the one image reader in the crate; a dispatch compares whatever it returns
            &crate::ops::docker::DockerImageFingerprint,
        ),
        Command::Gate { run: run_id } => gate::run(&ctx, run_id),
        Command::Promote { run: run_id, yes } => promote::run(&ctx, run_id, *yes),
        Command::Status {
            run: run_id,
            wait,
            kill_after,
        } => status::run(&ctx, run_id, *wait, *kill_after),
        Command::Attach { run: run_id } => attach::run(&ctx, run_id),
        // deliberately not `&ctx`: `ps` asks docker, never a manifest, so it works from anywhere
        Command::Ps { json } => ps::run(*json),
        Command::Experiment {
            tasks,
            repo,
            agent,
            resume,
            kill_after,
            selfcheck,
        } => experiment::run(
            &ctx,
            tasks,
            repo.as_deref(),
            agent.as_deref(),
            resume.as_deref(),
            *kill_after,
            *selfcheck,
        ),
        Command::ContainerEntrypoint => container_entrypoint::run(),
        Command::Prereqs => container_entrypoint::prereqs_only(),
        Command::AgentLaunch => agent_launch::run(),
    }
}

/// Resolve a task argument: a task id under `tasks_dir`, or a path to a dir / README.md.
pub fn resolve_task_arg(tasks_dir: &std::path::Path, task: &str) -> anyhow::Result<PathBuf> {
    let as_path = PathBuf::from(task);
    if as_path.is_dir() || as_path.is_file() {
        return Ok(as_path);
    }
    let candidate = tasks_dir.join(task);
    if candidate.is_dir() {
        return Ok(candidate);
    }
    bail!(
        "no task dir for {task:?}: tried {candidate} and the path {as_path}",
        candidate = candidate.display(),
        as_path = as_path.display()
    )
}

/// How many recent run ids the "no such run" hint lists.
pub const RECENT_RUN_HINTS: usize = 5;

/// Resolve a run argument for `status` / `gate` / `promote` / `attach`. Accepted forms, in order:
///
/// 1. the run-dir id under `runs_dir` (what the manifests record);
/// 2. the run's container name, `harness-<run id>` — the name `docker ps` shows;
/// 3. any other directory whose manifest `container` field equals the argument, so a future
///    container-naming change cannot break the argument.
///
/// On no match the error names the path that was tried and lists the newest run ids.
pub fn resolve_run_arg(resolved: &Resolved, arg: &str) -> anyhow::Result<PathBuf> {
    let runs_dir = resolved.runs_dir();
    let direct = resolved.run_dir(arg);
    if direct.is_dir() {
        return Ok(direct);
    }
    // `harness-<run id>`: retry with the prefix stripped. A run dir whose manifest is not readable
    // yet still counts — the id is unambiguous on its own.
    if let Some(run_id) = arg.strip_prefix(crate::ops::container::CONTAINER_PREFIX) {
        let candidate = runs_dir.join(run_id);
        if candidate.is_dir() {
            match Manifest::load(&candidate) {
                Ok(manifest) => {
                    if manifest.container == arg {
                        return Ok(candidate);
                    }
                }
                // no manifest (or an unreadable one): the stripped id is enough
                Err(_) => return Ok(candidate),
            }
        }
    }
    let recent = recent_runs(&runs_dir);
    let by_container = recent
        .iter()
        .find(|(dir, _)| Manifest::load(dir).is_ok_and(|manifest| manifest.container == arg))
        .map(|(dir, _)| dir.clone());
    if let Some(dir) = by_container {
        return Ok(dir);
    }
    bail!(
        "no such run: {} (nothing in {} matches {arg:?} as a run id, as a container name \
         harness-<run id>, or as a manifest container); `taskfmt ps` lists every run container on \
         this host; recent runs: {}",
        direct.display(),
        runs_dir.display(),
        if recent.is_empty() {
            "(none)".to_string()
        } else {
            recent
                .iter()
                .take(RECENT_RUN_HINTS)
                .map(|(_, id)| id.clone())
                .collect::<Vec<_>>()
                .join(", ")
        }
    )
}

/// Run ids under `runs_dir`, newest first: `(run dir, run id)` for every dir holding a manifest.
/// Runs and experiment dirs (`exp-*`, `repos`) share `runs_dir`, so a directory without a manifest
/// is not a run.
pub fn recent_runs(runs_dir: &Path) -> Vec<(PathBuf, String)> {
    let Ok(entries) = std::fs::read_dir(runs_dir) else {
        return Vec::new();
    };
    let mut runs: Vec<(PathBuf, String)> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|dir| dir.is_dir() && dir.join(MANIFEST_FILE).is_file())
        .filter_map(|dir| {
            let id = dir.file_name()?.to_string_lossy().to_string();
            Some((dir, id))
        })
        .collect();
    // run ids start with `YYYYMMDD-HHMMSS`, so reverse-lexicographic is newest first
    runs.sort_by(|a, b| b.1.cmp(&a.1));
    runs
}

/// Every task dir under `tasks_dir`, ordered by id.
pub fn all_task_dirs(tasks_dir: &std::path::Path) -> anyhow::Result<Vec<PathBuf>> {
    if !tasks_dir.is_dir() {
        bail!("tasks dir not found: {}", tasks_dir.display());
    }
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(tasks_dir)
        .with_context(|| format!("cannot read {}", tasks_dir.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && path.join("README.md").is_file())
        .collect();
    dirs.sort();
    Ok(dirs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_precedence_is_flag_then_env_then_discovery() {
        let cwd = std::env::current_dir().unwrap();
        // no choice at all: the relative default, which `Ctx::load` discovers by walking up
        assert_eq!(
            config_path(None, None),
            PathBuf::from(crate::config::MANIFEST_NAME)
        );
        // the env var is a choice, and is absolutized so discovery is bypassed
        assert_eq!(
            config_path(None, Some(PathBuf::from("env.toml"))),
            cwd.join("env.toml")
        );
        // the flag wins over the env var
        assert_eq!(
            config_path(
                Some(PathBuf::from("/flag/experiment.toml")),
                Some(PathBuf::from("/env/experiment.toml"))
            ),
            PathBuf::from("/flag/experiment.toml")
        );
    }

    #[test]
    fn an_explicit_config_flag_bypasses_discovery() {
        use clap::Parser as _;
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let named = root.join("elsewhere.toml");
        let cli = crate::cli::Cli::try_parse_from([
            "taskfmt",
            "--config",
            named.to_str().unwrap(),
            "status",
            "some-run",
        ])
        .unwrap();
        let ctx = Ctx::from_cli(&cli);
        assert_eq!(ctx.config_path, named);
        // and it is read as named: no ancestor of the cwd is consulted
        let err = ctx
            .load()
            .err()
            .map(|err| format!("{err:#}"))
            .expect("a named manifest that does not exist must not fall back to discovery");
        assert!(err.contains("cannot read experiment manifest"), "{err}");
        assert!(err.contains(&named.display().to_string()), "{err}");
    }

    #[test]
    fn task_arg_resolves_id_or_path() {
        let dir = tempfile::tempdir().unwrap();
        let tasks = dir.path().join("tasks");
        std::fs::create_dir_all(tasks.join("TASK-101")).unwrap();
        assert_eq!(
            resolve_task_arg(&tasks, "TASK-101").unwrap(),
            tasks.join("TASK-101")
        );
        let direct = dir.path().join("elsewhere");
        std::fs::create_dir_all(&direct).unwrap();
        assert_eq!(
            resolve_task_arg(&tasks, direct.to_str().unwrap()).unwrap(),
            direct
        );
        assert!(resolve_task_arg(&tasks, "TASK-999").is_err());
    }

    #[test]
    fn all_task_dirs_only_counts_packages() {
        let dir = tempfile::tempdir().unwrap();
        let tasks = dir.path().join("tasks");
        std::fs::create_dir_all(tasks.join("TASK-101")).unwrap();
        std::fs::create_dir_all(tasks.join("TASK-100")).unwrap();
        std::fs::create_dir_all(tasks.join("not-a-task")).unwrap();
        std::fs::write(tasks.join("TASK-100").join("README.md"), "x").unwrap();
        std::fs::write(tasks.join("TASK-101").join("README.md"), "x").unwrap();
        let dirs = all_task_dirs(&tasks).unwrap();
        let names: Vec<String> = dirs
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["TASK-100", "TASK-101"]);
    }

    /// A container that says nothing about itself beyond its name and its `/work` mount — the
    /// shape of every container launched before the `taskfmt.*` labels existed.
    fn pre_label(name: &str, work_mount: &str) -> ContainerInfo {
        ContainerInfo {
            name: name.to_string(),
            state: "running".to_string(),
            work_mount: Some(PathBuf::from(work_mount)),
            labels: Default::default(),
        }
    }

    /// A container carrying the labels this harness now launches with.
    fn labelled(name: &str, pairs: &[(&str, &str)]) -> ContainerInfo {
        ContainerInfo {
            name: name.to_string(),
            state: "running".to_string(),
            work_mount: None,
            labels: pairs
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        }
    }

    #[test]
    fn locate_run_dir_prefers_an_explicit_path_over_docker() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path().join("20260101-000000-x-TASK-001");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(run_dir.join(MANIFEST_FILE), "{}").unwrap();
        // an explicit path is the operator naming the run outright: no docker call is warranted
        let found = locate_run_dir_with(run_dir.to_str().unwrap(), |_| {
            panic!("docker inspect must not be consulted when the path is already a run dir")
        })
        .unwrap();
        assert_eq!(found.run_dir, std::fs::canonicalize(&run_dir).unwrap());
        assert_eq!(found.manifest, None);
    }

    #[test]
    fn locate_run_dir_accepts_the_path_of_a_manifest_json() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path().join("20260101-000000-x-TASK-001");
        std::fs::create_dir_all(&run_dir).unwrap();
        let manifest = run_dir.join(MANIFEST_FILE);
        std::fs::write(&manifest, "{}").unwrap();
        let found = locate_run_dir_with(manifest.to_str().unwrap(), |_| {
            panic!("docker inspect must not be consulted for a manifest path")
        })
        .unwrap();
        assert_eq!(found.run_dir, std::fs::canonicalize(&run_dir).unwrap());
        // a file that is not a run manifest is not a run
        let other = run_dir.join("notes.txt");
        std::fs::write(&other, "x").unwrap();
        assert!(locate_run_dir_with(other.to_str().unwrap(), |_| None).is_none());
    }

    #[test]
    fn locate_run_dir_tries_the_prefixed_container_name_before_the_bare_one() {
        let seen = std::cell::RefCell::new(Vec::new());
        let found = locate_run_dir_with("20260101-000000-x-TASK-001", |name| {
            seen.borrow_mut().push(name.to_string());
            (name == "harness-20260101-000000-x-TASK-001")
                .then(|| pre_label(name, "/runs/20260101-000000-x-TASK-001/workspace"))
        })
        .unwrap();
        assert_eq!(
            found.run_dir,
            PathBuf::from("/runs/20260101-000000-x-TASK-001")
        );
        // the prefixed name was tried first and hit, so the bare id was never tried
        assert_eq!(
            seen.into_inner(),
            vec!["harness-20260101-000000-x-TASK-001"]
        );
    }

    #[test]
    fn locate_run_dir_falls_back_to_the_bare_argument_as_a_container_name() {
        let found = locate_run_dir_with("20260101-000000-x-TASK-001", |name| {
            (name == "20260101-000000-x-TASK-001")
                .then(|| pre_label(name, "/runs/20260101-000000-x-TASK-001/workspace"))
        })
        .unwrap();
        assert_eq!(
            found.run_dir,
            PathBuf::from("/runs/20260101-000000-x-TASK-001")
        );
    }

    #[test]
    fn locate_run_dir_does_not_double_prefix_an_already_prefixed_container_name() {
        let seen = std::cell::RefCell::new(Vec::new());
        let found = locate_run_dir_with("harness-20260101-000000-x-TASK-001", |name| {
            seen.borrow_mut().push(name.to_string());
            None
        });
        assert!(found.is_none());
        assert_eq!(
            seen.into_inner(),
            vec!["harness-20260101-000000-x-TASK-001"]
        );
    }

    #[test]
    fn locate_run_dir_is_none_when_nothing_matches() {
        assert!(locate_run_dir_with("no-such-run", |_| None).is_none());
    }

    #[test]
    fn a_containers_labels_are_preferred_over_its_mounts() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_path = dir.path().join(MANIFEST_NAME);
        std::fs::write(&manifest_path, MINIMAL_MANIFEST).unwrap();
        let mut info = labelled(
            "harness-r",
            &[
                (container::LABEL_RUN_ID, "r"),
                (container::LABEL_RUN_DIR, "/runs/labelled"),
                (container::LABEL_MANIFEST, manifest_path.to_str().unwrap()),
            ],
        );
        // a mount that disagrees with the label: the label is the run's own statement about itself
        info.work_mount = Some(PathBuf::from("/runs/mounted/workspace"));
        let found = locate_run_dir_with("r", |_| Some(info.clone())).unwrap();
        assert_eq!(found.run_dir, PathBuf::from("/runs/labelled"));
        assert_eq!(found.manifest, Some(manifest_path));
    }

    #[test]
    fn a_manifest_label_short_circuits_the_upward_walk() {
        // the labelled manifest is *not* an ancestor of the run dir, so only the label can find it
        let elsewhere = tempfile::tempdir().unwrap();
        let manifest_path = std::fs::canonicalize(elsewhere.path())
            .unwrap()
            .join("named.toml");
        std::fs::write(&manifest_path, MINIMAL_MANIFEST).unwrap();
        let runs = tempfile::tempdir().unwrap();
        let run_dir = std::fs::canonicalize(runs.path()).unwrap().join("r");
        std::fs::create_dir_all(&run_dir).unwrap();
        let resolved = config_for_location(&RunLocation {
            run_dir: run_dir.clone(),
            manifest: Some(manifest_path.clone()),
        })
        .unwrap();
        assert_eq!(resolved.manifest, manifest_path);
        // without the label the same run dir has no manifest above it at all
        let err = config_for_location(&RunLocation {
            run_dir,
            manifest: None,
        })
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("no experiment.toml in it or any ancestor"),
            "{err}"
        );
    }

    /// The smallest manifest `ExperimentConfig::parse` accepts: `schema` and one agent profile are
    /// the only fields without a default (`config.rs`'s own `EXAMPLE` test fixture has the full
    /// documented shape; this only needs to load, not to dispatch anything).
    const MINIMAL_MANIFEST: &str = r#"
schema = "experiment/v1"
[agents.default]
profile = "x"
[agents.profiles.x]
kind = "claude"
image = "harness-claude:latest"
"#;

    /// A root holding a manifest and one run directory under `experiments/runs/`.
    fn root_with_run(id: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::write(root.join(MANIFEST_NAME), MINIMAL_MANIFEST).unwrap();
        let run_dir = root.join("experiments/runs").join(id);
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(run_dir.join(MANIFEST_FILE), "{}").unwrap();
        (dir, root, run_dir)
    }

    #[test]
    fn the_run_decides_the_manifest_even_when_the_process_has_a_good_one() {
        // two complete checkouts. The process is pointed at A; the argument names a run in B.
        let (_a, root_a, _) = root_with_run("20260101-000000-x-TASK-001");
        let (_b, root_b, run_b) = root_with_run("20260102-000000-x-TASK-002");
        let ctx = Ctx {
            config_path: root_a.join(MANIFEST_NAME),
            verbose: false,
            interaction: Interaction::new(true, true),
        };
        assert!(ctx.load().is_ok(), "the process manifest is perfectly good");
        let (resolved, run_dir) = load_run(&ctx, run_b.to_str().unwrap()).unwrap();
        assert_eq!(
            resolved.root, root_b,
            "the run's own manifest wins; the cwd's is not consulted"
        );
        assert_eq!(run_dir, run_b);
        assert_eq!(resolved.manifest, root_b.join(MANIFEST_NAME));
    }

    #[test]
    fn load_for_run_resolves_the_manifest_from_the_run_alone() {
        let (_dir, root, run_dir) = root_with_run("20260101-000000-x-TASK-001");
        let elsewhere = tempfile::tempdir().unwrap();
        let ctx = Ctx {
            config_path: elsewhere.path().join("experiment.toml"), // does not exist
            verbose: false,
            interaction: Interaction::new(true, true),
        };
        assert!(ctx.load().is_err());
        let resolved = load_for_run(&ctx, run_dir.to_str().unwrap()).unwrap();
        assert_eq!(resolved.root, root);
    }

    #[test]
    fn an_argument_that_names_no_run_falls_back_to_the_process_manifest() {
        let (_dir, root, run_dir) = root_with_run("20260101-000000-x-TASK-001");
        let ctx = Ctx {
            config_path: root.join(MANIFEST_NAME),
            verbose: false,
            interaction: Interaction::new(true, true),
        };
        // the run id alone names no directory here and no container anywhere: the process manifest
        // is what is left, and the shared resolver turns the id into the same run dir
        let (resolved, found) = load_run(&ctx, "20260101-000000-x-TASK-001").unwrap();
        assert_eq!(resolved.root, root);
        assert_eq!(found, run_dir);
    }

    #[test]
    fn load_run_error_names_both_attempts_and_says_what_to_do() {
        let elsewhere = tempfile::tempdir().unwrap();
        let ctx = Ctx {
            config_path: elsewhere.path().join("experiment.toml"),
            verbose: false,
            interaction: Interaction::new(true, true),
        };
        let err = load_run(&ctx, "no-such-run")
            .err()
            .map(|err| format!("{err:#}"))
            .unwrap();
        assert!(err.contains("cannot read experiment manifest"), "{err}");
        assert!(err.contains("cannot locate a run directory"), "{err}");
        // and it ends with the two things an operator can actually do next
        assert!(err.contains("taskfmt ps"), "{err}");
        assert!(err.contains("--config"), "{err}");
        assert!(err.contains("TASKFMT_CONFIG"), "{err}");
    }
}
