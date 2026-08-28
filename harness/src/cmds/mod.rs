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

use std::path::PathBuf;

use anyhow::{Context, bail};

use crate::cli::Cli;
use crate::config::{ExperimentConfig, Resolved};
use crate::interactive::Interaction;

/// Everything a command needs besides its own arguments.
pub struct Ctx {
    pub config_path: PathBuf,
    pub verbose: bool,
    pub interaction: Interaction,
}

impl Ctx {
    pub fn from_cli(cli: &Cli) -> Self {
        Self {
            config_path: cli.config.clone(),
            verbose: cli.verbose,
            interaction: Interaction::new(cli.auto, cli.yes),
        }
    }

    /// Load the experiment manifest and the path resolver rooted at its directory.
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
            selfcheck,
        } => experiment::run(
            &ctx,
            tasks,
            repo.as_deref(),
            agent.as_deref(),
            resume.as_deref(),
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
