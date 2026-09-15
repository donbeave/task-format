//! `taskfmt selftest` — thin entry over `taskfmt::selftest`. Needs no config and no secrets.

use crate::cmds::Ctx;

pub fn run(_ctx: &Ctx) -> anyhow::Result<i32> {
    taskfmt::selftest::console()
}
