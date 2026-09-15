//! In-container runtime command surface: entrypoint, prereqs, agent supervisor.

use clap::{Parser, Subcommand};
use taskfmt::cli_common::VerboseArgs;

#[derive(Parser, Debug)]
#[command(
    name = "taskfmt-runtime",
    version = taskfmt::VERSION,
    about = "In-container boot, prerequisites, and agent supervisor",
    long_about = None,
    arg_required_else_help = true,
    after_help = "Task validation (lint, init, status, verify) lives in the separate `taskfmt` binary."
)]
pub struct Cli {
    #[command(flatten)]
    pub verbose: VerboseArgs,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Container PID 1 (root): inner dockerd, agent seeding, prereqs, then the agent.
    #[command(name = "container-entrypoint")]
    ContainerEntrypoint,

    /// Container runtime prerequisites (root): inner postgres + seed restore.
    #[command(name = "prereqs")]
    Prereqs,

    /// Agent supervisor (as user `agent`): herdr server + one /work workspace + the agent pane.
    #[command(name = "agent-launch")]
    AgentLaunch,

    /// Read an API key on stdin and write codex auth.json (never argv).
    #[command(name = "codex-login")]
    CodexLogin,
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
