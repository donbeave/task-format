//! Host operator `taskfmt-host` binary: dispatch, lint, experiment orchestration.

use std::process::ExitCode;

use clap::Parser as _;
use taskfmt::cli::host::Cli;
use taskfmt::cmds;
use taskfmt::redact;

fn main() -> ExitCode {
    redact::init();
    let cli = Cli::parse();
    match cmds::dispatch_host(&cli) {
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
