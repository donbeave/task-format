//! In-container `taskfmt` binary: gate, entrypoint, and agent supervisor.

use std::process::ExitCode;

use clap::Parser as _;
use taskfmt::cli::container::Cli;
use taskfmt::cmds;
use taskfmt::redact;

fn main() -> ExitCode {
    redact::init();
    let cli = Cli::parse();
    match cmds::dispatch_container(&cli) {
        Ok(code) => ExitCode::from(code as u8),
        Err(err) => {
            redact::eemit(&format!("taskfmt: {err:#}"));
            for cause in err.chain().skip(1) {
                redact::eemit(&format!("  caused by: {cause}"));
            }
            ExitCode::from(1)
        }
    }
}
