mod lint;
mod status;
mod verify;

use crate::cli::{Cli, Command};

pub fn dispatch(cli: Cli) -> anyhow::Result<i32> {
    match cli.command {
        Command::Lint { task_dir, json } => lint::run(&task_dir, json),
        Command::Status {
            task_dir,
            progress,
            json,
        } => status::run(&task_dir, &progress, json),
        Command::Verify(options) => verify::run(options),
    }
}
