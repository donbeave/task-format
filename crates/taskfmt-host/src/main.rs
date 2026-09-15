//! Host operator `taskfmt-host` binary: dispatch, lint, experiment orchestration.

use std::process::ExitCode;

use clap::Parser as _;
use taskfmt::redact;
use taskfmt_host::{cli, cmds};

fn main() -> ExitCode {
    redact::init();
    let cli = cli::Cli::parse();
    match cmds::dispatch(&cli) {
        Ok(code) => ExitCode::from(code as u8),
        Err(err) => {
            redact::eemit(&format!("taskfmt-host: {err:#}"));
            for cause in err.chain().skip(1) {
                redact::eemit(&format!("  caused by: {cause}"));
            }
            ExitCode::from(1)
        }
    }
}
