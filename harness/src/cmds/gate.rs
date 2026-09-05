//! `taskfmt gate <RUN>` — the host gate: re-run verify on the workspace with trusted copies and
//! record the verdict in the manifest.

use std::path::Path;

use crate::cmds::Ctx;
use crate::config::Resolved;
use crate::runstate::Manifest;

pub fn run(ctx: &Ctx, run_id: &str) -> anyhow::Result<i32> {
    let _ = crate::cmds::load_run(ctx, run_id)?;
    anyhow::bail!(
        "host gate disabled: legacy runs do not provide an isolated immutable verifier candidate"
    )
}

/// Gate one run: trusted copies, the recorded base commit, progress from the run dir.
pub fn gate_run(
    _run_dir: &Path,
    _resolved: &Resolved,
    _manifest: &mut Manifest,
) -> anyhow::Result<bool> {
    anyhow::bail!(
        "host gate disabled: legacy runs do not provide an isolated immutable verifier candidate"
    )
}
