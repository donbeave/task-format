//! Command implementations. `dispatch` maps the parsed CLI to a module and a process exit code.

pub mod agent_launch;
pub mod attach;
pub mod build_images;
pub mod container_entrypoint;
pub mod experiment;
pub mod gate;
pub mod lint;
pub mod preload;
pub mod progress_init;
pub mod promote;
pub mod repo;
pub mod run;
pub mod selfcheck;
pub mod selftest;
pub mod status;
pub mod verify;

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use crate::cli::Cli;
use crate::config::{ExperimentConfig, Resolved};
use crate::interactive::Interaction;
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
        let (cfg, root) = ExperimentConfig::load(&self.config_path)?;
        Ok(Resolved::new(&root, cfg))
    }
}

/// Dispatch one parsed CLI. Returns the process exit code.
pub fn dispatch(cli: &Cli) -> anyhow::Result<i32> {
    let ctx = Ctx::from_cli(cli);
    use crate::cli::{Command, RepoCmd};
    match &cli.command {
        Command::Lint { tasks } => lint::run(&ctx, tasks),
        Command::ProgressInit { task, out } => progress_init::run(&ctx, task, out.as_deref()),
        Command::Selftest => selftest::run(&ctx),
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
        ),
        Command::Gate { run: run_id } => gate::run(&ctx, run_id),
        Command::Promote { run: run_id, yes } => promote::run(&ctx, run_id, *yes),
        Command::Status {
            run: run_id,
            wait,
            kill_after,
        } => status::run(&ctx, run_id, *wait, *kill_after),
        Command::Attach { run: run_id } => attach::run(&ctx, run_id),
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
         harness-<run id>, or as a manifest container); recent runs: {}",
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
}
