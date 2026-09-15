//! In-container runtime command dispatch.

pub mod agent_launch;
pub mod container_entrypoint;

use crate::cli::Cli;

/// Dispatch the in-container runtime CLI. Returns the process exit code.
pub fn dispatch(cli: &Cli) -> anyhow::Result<i32> {
    use crate::cli::Command;
    match &cli.command {
        Command::ContainerEntrypoint => container_entrypoint::run(),
        Command::Prereqs => container_entrypoint::prereqs_only(),
        Command::AgentLaunch => agent_launch::run(),
        Command::CodexLogin => container_entrypoint::codex_login(),
    }
}
