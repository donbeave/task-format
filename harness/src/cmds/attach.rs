//! `taskfmt attach <RUN>` — re-attach to the live agent TUI. Foreground `docker exec`, replacing
//! this process so the terminal hands over cleanly. Detach: ctrl+b q — never ctrl+c.

use std::process::Command;

use anyhow::Context;
use std::os::unix::process::CommandExt;

use crate::cmds::Ctx;
use crate::redact;
use crate::runstate::Manifest;

pub fn run(ctx: &Ctx, run_id: &str) -> anyhow::Result<i32> {
    let resolved = crate::cmds::load_for_run(ctx, run_id)?;
    let run_dir = crate::cmds::resolve_run_arg(&resolved, run_id)?;
    let manifest = Manifest::load(&run_dir)?;
    if !crate::ops::docker::is_running(&manifest.container) {
        redact::eemit(&format!(
            "container {} is stopped — starting it",
            manifest.container
        ));
        crate::ops::docker::start(&manifest.container)
            .context("docker start (restart relaunches the agent from the session)")?;
    }
    redact::eemit(&format!(
        "attaching to {} (session {:?}); detach with ctrl+b q — never ctrl+c",
        manifest.container, manifest.session_id
    ));
    let err = Command::new("docker")
        .args([
            "exec",
            "-it",
            "-u",
            "agent",
            "-e",
            &format!(
                "TERM={}",
                std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".into())
            ),
            &manifest.container,
            "herdr",
        ])
        .exec();
    // exec only returns on failure
    Err(err).context("docker exec herdr")
}
