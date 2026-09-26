mod acceptance;
mod cli;
mod cmds;
mod gate;
mod lint;
mod ops;
mod progress;
mod progress_view;
mod taskfile;
mod verifycfg;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    match cmds::dispatch(cli) {
        Ok(code) => ExitCode::from(code as u8),
        Err(error) => {
            eprintln!("taskfmt: {error:#}");
            ExitCode::from(1)
        }
    }
}
