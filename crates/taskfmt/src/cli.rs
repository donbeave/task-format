//! In-container validation command surface: init, status, lint, verify.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "taskfmt",
    version = taskfmt::VERSION,
    about = "In-container task init, progress, lint, and verification",
    after_help = "Container boot and agent supervision live in the `taskfmt-runtime` binary. \
                  Host orchestration lives in `taskfmt-host`."
)]
pub struct Cli {
    /// Verbose: echo every external command invocation (scrubbed).
    #[arg(short = 'v', long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create progress.md once from the task README (fails if it already exists).
    Init {
        /// Task package directory (default: $TASKFMT_TASK_DIR or /task).
        #[arg(long)]
        task_dir: Option<PathBuf>,
        /// Output progress file (default: $PROGRESS_FILE or /progress/progress.md).
        #[arg(long)]
        out: Option<PathBuf>,
    },

    /// Parse and validate progress.md; print derived coordination state.
    Status {
        /// Emit JSON instead of a plain key=value line.
        #[arg(long)]
        json: bool,
        /// Task package directory (default: $TASKFMT_TASK_DIR or /task).
        #[arg(long)]
        task_dir: Option<PathBuf>,
        /// Progress file (default: $PROGRESS_FILE or /progress/progress.md).
        #[arg(long)]
        progress: Option<PathBuf>,
    },

    /// Lint task packages (default: $TASKFMT_TASK_DIR or /task).
    Lint {
        /// Emit one stable JSON report per package (NDJSON).
        #[arg(long)]
        json: bool,
        /// Task package directories or README.md paths.
        paths: Vec<PathBuf>,
    },

    /// The completion gate: exit 0 AND last stdout line "DONE" <=> pass.
    Verify {
        /// Repository root the gate runs in (default: TASKFMT_ROOT, git toplevel of cwd, cwd).
        #[arg(long)]
        root: Option<PathBuf>,
        /// Directory holding README.md + verify.toml (default: TASKFMT_TASK_DIR, /task, cwd).
        #[arg(long)]
        task_dir: Option<PathBuf>,
        /// Progress file. Empty string disables the progress check.
        #[arg(long)]
        progress: Option<String>,
        /// Disable the progress check (same as --progress "").
        #[arg(long, conflicts_with = "progress")]
        no_progress: bool,
        /// Scope base ref. Order: --base > TASKFMT_BASE > base_ref in verify.toml > "baseline".
        #[arg(long)]
        base: Option<String>,
        /// Directory for per-check logs (default: a fresh temp dir).
        #[arg(long)]
        log_dir: Option<PathBuf>,
        /// Stop at the first failing check.
        #[arg(long)]
        fail_fast: bool,
    },
}
