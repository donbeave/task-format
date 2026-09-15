//! In-container command surface: gate, runtime entrypoint, and agent supervisor only.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "taskfmt",
    version = crate::VERSION,
    about = "In-container task lint, verification, and runtime",
    after_help = "Host-side dispatch and experiment orchestration live in the `taskfmt-host` binary."
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

    /// Print the content fingerprint baked into this binary.
    Fingerprint {
        /// Recompute the digest over a crate directory instead of printing the compiled-in value.
        #[arg(long)]
        path: Option<PathBuf>,
    },

    /// Container PID 1 (root): inner dockerd, agent seeding, prereqs, then the agent.
    ContainerEntrypoint,

    /// Container runtime prerequisites (root): inner postgres + seed restore.
    Prereqs,

    /// Agent supervisor (as user `agent`): herdr server + one /work workspace + the agent pane.
    AgentLaunch,

    /// Read an API key on stdin and write codex auth.json (never argv).
    CodexLogin,
}
