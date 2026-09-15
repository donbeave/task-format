//! In-container `taskfmt-runtime` binary: PID 1, prereqs, and agent supervisor.

use std::process::ExitCode;

use clap::Parser as _;
use taskfmt::redact;
use taskfmt_runtime::{cli, cmds};

fn main() -> ExitCode {
    redact::init();
    let cli = cli::Cli::parse();
    match cmds::dispatch(&cli) {
        Ok(code) => ExitCode::from(code as u8),
        Err(err) => {
            redact::eemit(&format!("taskfmt-runtime: {err:#}"));
            for cause in err.chain().skip(1) {
                redact::eemit(&format!("  caused by: {cause}"));
            }
            ExitCode::from(1)
        }
    }
}
