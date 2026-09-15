//! Command-line surface (clap derive). Global flags live in [`GlobalArgs`] and flatten into [`Cli`].

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use taskfmt::cli_common::{ConfigArg, ConfirmArgs, VerboseArgs};

/// Global flags shared by every host subcommand.
#[derive(Args, Debug, Clone, Default)]
pub struct GlobalArgs {
    #[command(flatten)]
    pub config: ConfigArg,
    #[command(flatten)]
    pub confirm: ConfirmArgs,
    #[command(flatten)]
    pub verbose: VerboseArgs,
}

#[derive(Parser, Debug)]
#[command(
    name = "taskfmt-host",
    version = taskfmt::VERSION,
    about = "Filesystem projects, groups, task contracts, and verified task execution",
    long_about = None,
    arg_required_else_help = true,
    after_help = "Read-only commands never prompt. Mutating commands (run, experiment, repo, \
                  promote, preload, build-images) need --auto or --yes when stdin is not a terminal. \
                  In-container validation lives in the separate `taskfmt` binary baked into harness images."
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum AgentFilter {
    Claude,
    Codex,
    Cursor,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum AgentKindArg {
    Claude,
    Codex,
    Cursor,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Lint task packages under the configured tasks dir.
    Lint {
        /// Emit one stable JSON report per package (NDJSON).
        #[arg(long, action = clap::ArgAction::SetTrue)]
        json: bool,
        /// Task IDs or directories. Empty = every task dir in tasks_dir.
        #[arg(value_name = "TASK")]
        tasks: Vec<String>,
    },

    /// Prove lint, progress init, and the gate on the bundled corpus.
    Selftest,

    /// Prove a task package's gate: RED on the untouched baseline, GREEN on the reference (D13).
    /// Exit 0 only on SELFCHECK RESULT PASS; 1 FAIL; 64 usage; 66 missing input; 69 no verdict
    /// (a focused command was not runnable: rc 126/127); 70 internal error.
    Selfcheck {
        /// Task package dir holding verify.toml (+ README.md).
        #[arg(value_name = "TASK")]
        task: PathBuf,
        /// Git repository checked out at the trusted base commit (never mutated: phases run in a
        /// scratch copy under TMPDIR).
        #[arg(value_name = "WORKSPACE")]
        workspace: PathBuf,
        /// Scope base ref. Order: --base > TASKFMT_BASE > base_ref in verify.toml > "baseline".
        #[arg(long, env = "TASKFMT_BASE")]
        base: Option<String>,
        /// Reference solution: a directory mirrored over the tree, or a .patch/.diff file.
        /// Absent: the oracle phase is SKIPPED.
        #[arg(long, value_name = "PATH")]
        reference: Option<PathBuf>,
        /// Retain the scratch copy (its path is printed).
        #[arg(long, action = clap::ArgAction::SetTrue)]
        keep: bool,
    },

    /// Build the harness container images.
    BuildImages {
        /// Which agent image to layer on harness-base.
        #[arg(long, value_enum, default_value = "all")]
        agent: AgentFilter,
        /// Pass --no-cache to every docker build.
        #[arg(long, action = clap::ArgAction::SetTrue)]
        no_cache: bool,
    },

    /// Bake the postgres prereq image: pull, pin the digest, save the tarball.
    Preload,

    /// GitHub repository lifecycle for an experiment.
    Repo {
        #[command(subcommand)]
        cmd: RepoCmd,
    },

    /// Dispatch ONE task to ONE fresh headed container.
    Run {
        /// Task ID (e.g. TASK-101).
        #[arg(long)]
        task: String,
        /// Repository to clone. Absent: create a new disposable repo (after confirmation).
        #[arg(long)]
        repo: Option<String>,
        /// Agent profile name from experiment.toml. Default: agents.default.
        #[arg(long)]
        agent: Option<String>,
        /// Override the profile model.
        #[arg(long)]
        model: Option<String>,
        /// Override the profile effort.
        #[arg(long)]
        effort: Option<String>,
        /// Stay attached: poll status, then gate and report.
        #[arg(long, action = clap::ArgAction::SetTrue)]
        wait: bool,
        /// Minutes after which a still-running run is killed (default: runtime.kill_after_min).
        #[arg(long)]
        kill_after: Option<u64>,
        /// Record the run under experiments/runs/<ID>/ for an experiment.
        #[arg(long)]
        exp: Option<String>,
        /// Run the D13 gate selfcheck (nop + polarity) on the built workspace after lint; refuse
        /// to dispatch on FAIL or NOVERDICT. Off by default: it runs the fixture's toolchain on
        /// the host (container-mode selfcheck is pending).
        #[arg(long, action = clap::ArgAction::SetTrue)]
        selfcheck: bool,
    },

    /// Host gate for one run: re-run verification in the caller-provided sandbox.
    Gate {
        /// Run id, container name, run directory, or manifest.json path.
        #[arg(value_name = "RUN")]
        run: String,
    },

    /// Push the exact tree recorded by a passing gate.
    Promote {
        /// Run id, container name, run directory, or manifest.json path.
        #[arg(value_name = "RUN")]
        run: String,
        /// Skip the confirmation (still refuses on gate FAIL).
        #[arg(long, action = clap::ArgAction::SetTrue)]
        yes: bool,
    },

    /// Completion detection for one run, from outside the container.
    Status {
        /// Run id, container name, run directory, or manifest.json path.
        #[arg(value_name = "RUN")]
        run: String,
        /// Poll until the run reaches a terminal state.
        #[arg(long, action = clap::ArgAction::SetTrue)]
        wait: bool,
        /// Minutes before a still-running agent is killed with `/goal clear`.
        #[arg(long)]
        kill_after: Option<u64>,
    },

    /// Re-attach to a run's live agent TUI (detach: ctrl+b q — never ctrl+c).
    Attach {
        /// Run id, container name, run directory, or manifest.json path.
        #[arg(value_name = "RUN")]
        run: String,
    },

    /// List the run containers on this host. Read-only, and needs no manifest: it asks docker.
    Ps {
        /// One JSON object per line instead of the table.
        #[arg(long, action = clap::ArgAction::SetTrue)]
        json: bool,
    },

    /// Full experiment loop: repo, then run -> gate -> promote per selected task.
    Experiment {
        /// Task selection: "all", "1-3,5", "TASK-002..TASK-004", "TASK-101".
        #[arg(long, value_delimiter = ',')]
        tasks: Vec<String>,
        /// Existing repo to run against. Absent: create a disposable one (after confirmation).
        #[arg(long)]
        repo: Option<String>,
        /// External baseline/reference corpus to validate before creating or mutating the runtime repo.
        #[arg(long, value_name = "PATH")]
        proof_corpus: Option<PathBuf>,
        /// Agent profile name from experiment.toml. Default: agents.default.
        #[arg(long)]
        agent: Option<String>,
        /// Override the profile model for every task in the batch.
        #[arg(long)]
        model: Option<String>,
        /// Override the profile effort for every task in the batch.
        #[arg(long)]
        effort: Option<String>,
        /// Resume an interrupted experiment: skip tasks already recorded passed.
        #[arg(long)]
        resume: Option<String>,
        /// Minutes after which a still-running run is killed (default: runtime.kill_after_min).
        #[arg(long)]
        kill_after: Option<u64>,
        /// Run the D13 gate selfcheck (nop + polarity) before each dispatch; refuse on FAIL or
        /// NOVERDICT. Off by default (host toolchain; container-mode selfcheck is pending).
        #[arg(long, action = clap::ArgAction::SetTrue)]
        selfcheck: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum RepoCmd {
    /// Create a private GitHub repo and bootstrap it (empty signed commit on main).
    Create {
        /// Repository name. Default: repo_prefix + UTC timestamp.
        #[arg(long)]
        name: Option<String>,
    },
    /// Delete a private GitHub repo previously created by this tool.
    Delete {
        #[arg(long)]
        name: Option<String>,
        #[arg(long, action = clap::ArgAction::SetTrue)]
        yes: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn version_contains_package_version_and_commit_sha() {
        assert_eq!(Cli::command().get_version(), Some(taskfmt::VERSION));
        assert_eq!(
            taskfmt::VERSION,
            format!(
                "{} (git {})",
                env!("CARGO_PKG_VERSION"),
                taskfmt::GIT_COMMIT_SHA
            )
        );
    }
}
