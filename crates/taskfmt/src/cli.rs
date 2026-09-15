//! In-container validation command surface: init, status, lint, verify.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use taskfmt::cli_common::{ProgressFileArg, TaskDirArg, VerboseArgs};

#[derive(Parser, Debug)]
#[command(
    name = "taskfmt",
    version = taskfmt::VERSION,
    about = "In-container task init, progress, lint, and verification",
    long_about = None,
    arg_required_else_help = true,
    after_help = "Container boot and agent supervision live in the `taskfmt-runtime` binary. \
                  Host orchestration lives in `taskfmt-host`."
)]
pub struct Cli {
    #[command(flatten)]
    pub verbose: VerboseArgs,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create progress.md once from the task README (fails if it already exists).
    Init {
        #[command(flatten)]
        task: TaskDirArg,
        /// Output progress file (default: $PROGRESS_FILE or /progress/progress.md).
        #[arg(long, env = "PROGRESS_FILE")]
        out: Option<PathBuf>,
    },

    /// Parse and validate progress.md; print derived coordination state.
    Status {
        /// Emit JSON instead of a plain key=value line.
        #[arg(long, action = clap::ArgAction::SetTrue)]
        json: bool,
        #[command(flatten)]
        task: TaskDirArg,
        #[command(flatten)]
        progress: ProgressFileArg,
    },

    /// Lint task packages (default: $TASKFMT_TASK_DIR or /task).
    Lint {
        /// Emit one stable JSON report per package (NDJSON).
        #[arg(long, action = clap::ArgAction::SetTrue)]
        json: bool,
        /// Task package directories or README.md paths.
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,
    },

    /// The completion gate: exit 0 AND last stdout line "DONE" <=> pass.
    Verify(VerifyOptions),
}

/// Options for [`Command::Verify`].
#[derive(Args, Debug)]
pub struct VerifyOptions {
    /// Repository root the gate runs in (default: TASKFMT_ROOT, git toplevel of cwd, cwd).
    #[arg(long, env = "TASKFMT_ROOT")]
    pub root: Option<PathBuf>,
    /// Directory holding README.md + verify.toml (default: TASKFMT_TASK_DIR, /task, cwd).
    #[arg(long, env = "TASKFMT_TASK_DIR")]
    pub task_dir: Option<PathBuf>,
    /// Progress file. Empty string disables the progress check.
    #[arg(long, env = "PROGRESS_FILE", group = "progress_check")]
    pub progress: Option<String>,
    /// Disable the progress check (same as --progress "").
    #[arg(long, group = "progress_check", action = clap::ArgAction::SetTrue)]
    pub no_progress: bool,
    /// Scope base ref. Order: --base > TASKFMT_BASE > base_ref in verify.toml > "baseline".
    #[arg(long, env = "TASKFMT_BASE")]
    pub base: Option<String>,
    /// Directory for per-check logs (default: a fresh temp dir).
    #[arg(long, value_name = "DIR")]
    pub log_dir: Option<PathBuf>,
    /// Stop at the first failing check.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub fail_fast: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
