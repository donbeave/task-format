//! In-container validation command dispatch.

pub mod init;
pub mod lint;
pub mod progress_status;
pub mod verify;

use crate::cli::Cli;

/// Dispatch the in-container validation CLI. Returns the process exit code.
pub fn dispatch(cli: &Cli) -> anyhow::Result<i32> {
    use crate::cli::Command;
    match &cli.command {
        Command::Init { task_dir, out } => init::run(task_dir.as_deref(), out.as_deref()),
        Command::Status {
            json,
            task_dir,
            progress,
        } => progress_status::run(*json, task_dir.as_deref(), progress.as_deref()),
        Command::Lint { json, paths } => lint::run_paths(*json, paths),
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
    }
}
