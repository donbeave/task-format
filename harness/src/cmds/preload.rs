//! `taskfmt preload` — pull, digest-pin and save the postgres prereq image.

use crate::cmds::Ctx;
use crate::ops::images;

pub fn run(ctx: &Ctx) -> anyhow::Result<i32> {
    ctx.interaction.require_consent_source("preload")?;
    let resolved = ctx.load()?;
    images::preload(&resolved.harness_dir())?;
    Ok(0)
}
