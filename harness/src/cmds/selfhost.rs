//! Thin dispatch for `taskfmt selfhost` (plan §4.6.1): one `run`, one `match`, no business logic.
//!
//! Its whole job is to be the file §8.1's carve-out names, so the subcommand enum under
//! `harness/src/selfhost/` can grow without reopening a fenced file.

use crate::cmds::Ctx;
use crate::selfhost::cli::SelfhostCmd;

pub fn run(ctx: &Ctx, cmd: &SelfhostCmd) -> anyhow::Result<i32> {
    match cmd {
        SelfhostCmd::Status(args) => crate::selfhost::status::run(ctx, args),
        SelfhostCmd::Record(args) => crate::selfhost::record::run(ctx, args),
    }
}
