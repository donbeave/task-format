//! Shared [`clap::Args`] groups reused across taskfmt binaries.

use std::path::PathBuf;

use clap::Args;

/// `-v` / `--verbose` on any subcommand.
#[derive(Args, Debug, Clone, Default)]
pub struct VerboseArgs {
    /// Echo every external command invocation (scrubbed).
    #[arg(short, long, global = true, action = clap::ArgAction::SetTrue)]
    pub verbose: bool,
}

/// Task package directory (`$TASKFMT_TASK_DIR`, default resolved at dispatch).
#[derive(Args, Debug, Clone, Default)]
pub struct TaskDirArg {
    /// Task package directory (default: $TASKFMT_TASK_DIR or /task).
    #[arg(long, env = "TASKFMT_TASK_DIR")]
    pub task_dir: Option<PathBuf>,
}

/// Progress file path (`$PROGRESS_FILE`, default resolved at dispatch).
#[derive(Args, Debug, Clone, Default)]
pub struct ProgressFileArg {
    /// Progress file (default: $PROGRESS_FILE or /progress/progress.md).
    #[arg(long, env = "PROGRESS_FILE")]
    pub progress: Option<PathBuf>,
}

/// Host/global experiment manifest selection.
#[derive(Args, Debug, Clone, Default)]
pub struct ConfigArg {
    /// Path to the experiment manifest (default: $TASKFMT_CONFIG, else nearest `experiment.toml`).
    #[arg(long, global = true, env = "TASKFMT_CONFIG")]
    pub config: Option<PathBuf>,
}

/// Host confirmation flags (`--auto` and `--yes` are equivalent).
#[derive(Args, Debug, Clone, Default)]
pub struct ConfirmArgs {
    /// Assume yes for every confirmation and print the plan line.
    #[arg(long, global = true, action = clap::ArgAction::SetTrue)]
    pub auto: bool,
    /// Alias of `--auto`: skip confirmations.
    #[arg(long, global = true, action = clap::ArgAction::SetTrue)]
    pub yes: bool,
}
