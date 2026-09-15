//! In-container validation command dispatch.

pub mod init;
pub mod lint;
pub mod progress_status;
pub mod verify;

use crate::cli::{Cli, Command};

/// Dispatch the in-container validation CLI. Returns the process exit code.
pub fn dispatch(cli: &Cli) -> anyhow::Result<i32> {
    match &cli.command {
        Command::Init { task, out } => init::run(task.task_dir.as_deref(), out.as_deref()),
        Command::Status {
            json,
            task,
            progress,
        } => progress_status::run(
            *json,
            task.task_dir.as_deref(),
            progress.progress.as_deref(),
        ),
        Command::Lint { json, paths } => lint::run_paths(*json, paths),
        Command::Verify(opts) => verify::run(
            opts.root.as_deref(),
            opts.task_dir.as_deref(),
            if opts.no_progress {
                Some(String::new())
            } else {
                opts.progress.clone()
            },
            opts.base.clone(),
            opts.log_dir.as_deref(),
            opts.fail_fast,
        ),
    }
}
