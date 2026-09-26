use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "taskfmt",
    version,
    about = "Validate task packages, read progress, and run local checks",
    arg_required_else_help = true,
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Validate one task package without executing its checks.
    Lint {
        #[arg(value_name = "TASK-DIR")]
        task_dir: PathBuf,
        /// Emit one JSON report.
        #[arg(long)]
        json: bool,
    },
    /// Read and validate a caller-maintained progress file.
    Status {
        #[arg(long, required = true, value_name = "DIR")]
        task_dir: PathBuf,
        #[arg(long, required = true, value_name = "FILE")]
        progress: PathBuf,
        /// Emit JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Validate a task and run its declared local checks.
    Verify(VerifyOptions),
}

#[derive(Args, Debug)]
pub struct VerifyOptions {
    /// Git workspace to check.
    #[arg(long, required = true, value_name = "DIR")]
    pub root: PathBuf,
    /// Directory containing README.md and verify.toml.
    #[arg(long, required = true, value_name = "DIR")]
    pub task_dir: PathBuf,
    /// Caller-maintained progress file; required for full verification.
    #[arg(
        long,
        required_unless_present = "no_progress",
        conflicts_with = "no_progress",
        value_name = "FILE"
    )]
    pub progress: Option<PathBuf>,
    /// Run task, scope, and declared checks without asserting task completion.
    #[arg(long)]
    pub no_progress: bool,
    /// Required Git commit or ref used for scope checks.
    #[arg(long, required = true, value_name = "COMMIT")]
    pub base: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_has_only_three_functional_commands_and_no_help_subcommand() {
        let command = Cli::command();
        command.clone().debug_assert();
        let names: Vec<_> = command
            .get_subcommands()
            .map(|subcommand| subcommand.get_name().to_string())
            .collect();
        assert_eq!(names, ["lint", "status", "verify"]);
        assert!(Cli::try_parse_from(["taskfmt", "help"]).is_err());
        assert!(Cli::try_parse_from(["taskfmt", "init"]).is_err());
    }
}
