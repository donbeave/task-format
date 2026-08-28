//! `taskfmt selftest` — thin entry over `crate::selftest`. Needs no config and no secrets.

use crate::cmds::Ctx;

pub fn run(_ctx: &Ctx) -> anyhow::Result<i32> {
    crate::selftest::console()
}
