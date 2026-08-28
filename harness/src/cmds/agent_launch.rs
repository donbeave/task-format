//! `taskfmt agent-launch` — agent supervisor, runs as user `agent` (exec'd from the entrypoint via
//! gosu). Headless herdr server + one /work workspace; the agent runs in the root pane under
//! script(1) so /out/tui.log holds the raw terminal stream from the first byte. docker stop →
//! SIGTERM here → graceful `herdr server stop` (writes session.json).

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};

use crate::ops::signals;
use crate::redact;

const OUT: &str = "/out";
const WORKSPACE: &str = "/work";

pub fn run() -> anyhow::Result<i32> {
    let _flags = signals::install_terminate_flag();
    let agent_cmd = std::env::var("AGENT_CMD").context("AGENT_CMD not set")?;
    let agent_kind = std::env::var("AGENT_KIND").context("AGENT_KIND not set")?;

    // 1. headless herdr server (does not daemonize) — keep its pid, log to /out
    let server_log = std::fs::File::create(format!("{OUT}/herdr-server.log"))
        .context("creating /out/herdr-server.log")?;
    let mut server = Command::new("herdr")
        .arg("server")
        .stdout(Stdio::from(server_log.try_clone()?))
        .stderr(Stdio::from(server_log))
        .spawn()
        .context("spawning herdr server")?;
    if !wait_server(Duration::from_secs(10)) {
        redact::eemit("agent-launch: herdr server did not come up — see /out/herdr-server.log");
        let _ = server.kill();
        return Ok(1);
    }

    // 2. one workspace rooted at /work → root pane id
    let pane = match workspace_pane() {
        Ok(pane) => pane,
        Err(err) => {
            redact::eemit(&format!(
                "agent-launch: no pane id from workspace create: {err:#}"
            ));
            let _ = Command::new("herdr").arg("server").arg("stop").status();
            let _ = server.kill();
            return Ok(1);
        }
    };
    std::fs::write(format!("{OUT}/pane-id"), format!("{pane}\n"))
        .context("writing /out/pane-id")?;

    // 3. agent under script(1): /out/tui.log = raw stream from byte 0; HERDR_AGENT tells herdr
    //    which screen manifest to use behind the wrapper
    let wrapped = format!(
        "HERDR_AGENT={agent_kind} exec script -qfec {} /out/tui.log",
        shell_quote(&agent_cmd)
    );
    redact::emit(&format!("agent-launch: pane {pane} ← {agent_cmd}"));
    crate::ops::check(
        Command::new("herdr")
            .args(["pane", "run", &pane, &wrapped])
            .env("HERDR_SESSION", "agent"),
        "herdr pane run",
    )?;

    // 4. supervise: SIGTERM (docker stop) → graceful server stop, then clean exit
    loop {
        if signals::sleep_until_terminate(Duration::from_secs(1)) {
            let _ = Command::new("herdr")
                .args(["server", "stop"])
                .stdout(Stdio::null())
                .status();
            let _ = server.kill();
            return Ok(0);
        }
        if let Ok(Some(status)) = server.try_wait() {
            redact::emit(&format!("agent-launch: herdr server exited ({status})"));
            return Ok(status.code().unwrap_or(0));
        }
    }
}

fn wait_server(timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if Command::new("herdr")
            .arg("status")
            .env("HERDR_SESSION", "agent")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

/// `herdr workspace create --cwd /work --label task --no-focus` → `.result.root_pane.pane_id`.
fn workspace_pane() -> anyhow::Result<String> {
    if !Path::new(WORKSPACE).is_dir() {
        bail!("{WORKSPACE} is not mounted");
    }
    let captured = crate::ops::capture(
        Command::new("herdr")
            .args([
                "workspace",
                "create",
                "--cwd",
                WORKSPACE,
                "--label",
                "task",
                "--no-focus",
            ])
            .env("HERDR_SESSION", "agent"),
    )
    .context("herdr workspace create")?;
    if !captured.ok() {
        bail!("herdr workspace create failed: {}", captured.stderr.trim());
    }
    let parsed: serde_json::Value =
        serde_json::from_str(captured.stdout.trim()).context("parsing workspace create output")?;
    let pane = parsed
        .pointer("/result/root_pane/pane_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    if pane.is_empty() || pane == "null" {
        bail!("no pane id in workspace create output");
    }
    Ok(pane)
}

/// Single-word quote for the `script -qfec <cmd>` wrapper (POSIX-safe: '…' with '\'' escapes).
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn shell_quote_survives_embedded_quotes() {
        let quoted = super::shell_quote("claude -m 'sonnet' --add-dir /task");
        assert_eq!(quoted, r"'claude -m '\''sonnet'\'' --add-dir /task'");
    }
}
