//! In-container runtime command surface: entrypoint, prereqs, agent supervisor.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "taskfmt-runtime",
    version = taskfmt::VERSION,
    about = "In-container boot, prerequisites, and agent supervisor",
    after_help = "Task validation (lint, init, status, verify) lives in the separate `taskfmt` binary."
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
    /// Container PID 1 (root): inner dockerd, agent seeding, prereqs, then the agent.
    ContainerEntrypoint,

    /// Container runtime prerequisites (root): inner postgres + seed restore.
    Prereqs,

    /// Agent supervisor (as user `agent`): herdr server + one /work workspace + the agent pane.
    AgentLaunch,

    /// Read an API key on stdin and write codex auth.json (never argv).
    CodexLogin,
}
