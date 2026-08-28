//! `taskfmt build-images` — build `harness-taskfmt` → `harness-base` → agent images.

use crate::cmds::Ctx;
use crate::ops::images::{self, AgentFilter};

impl From<crate::cli::AgentFilter> for AgentFilter {
    fn from(value: crate::cli::AgentFilter) -> Self {
        match value {
            crate::cli::AgentFilter::Claude => AgentFilter::Claude,
            crate::cli::AgentFilter::Codex => AgentFilter::Codex,
            crate::cli::AgentFilter::All => AgentFilter::All,
        }
    }
}

pub fn run(ctx: &Ctx, filter: AgentFilter, no_cache: bool) -> anyhow::Result<i32> {
    // mutating command: requires --auto/--yes when stdin is not a terminal
    ctx.interaction.require_consent_source("build-images")?;
    let resolved = ctx.load()?;
    images::build_images(&resolved.cfg, &resolved, filter, no_cache, ctx.verbose)?;
    Ok(0)
}
