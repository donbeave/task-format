//! herdr — the only terminal multiplexer allowed here (hard rule). All calls run inside the run
//! container as user `agent` with `HERDR_SESSION=agent`, so the socket resolves.

use anyhow::Context;

use crate::redact;
use crate::runstate::Manifest;

use super::docker;

const HERDR: &str = "herdr";

fn env() -> Vec<(String, String)> {
    vec![("HERDR_SESSION".to_string(), "agent".to_string())]
}

/// `herdr agent wait <target> --until idle --timeout MS`
pub fn wait_idle(manifest: &Manifest, timeout_ms: u64) -> anyhow::Result<bool> {
    let args = vec![
        HERDR.to_string(),
        "agent".to_string(),
        "wait".to_string(),
        manifest.agent_name.clone(),
        "--until".to_string(),
        "idle".to_string(),
        "--timeout".to_string(),
        timeout_ms.to_string(),
    ];
    let out = docker::exec(&manifest.container, Some("agent"), &env(), &args, false)?;
    Ok(out.ok())
}

/// Every status herdr 0.8.2 can classify an agent into (the `herdr agent wait --until` possible
/// values). `unknown` is how herdr reports an agent whose process is gone (the pane is back at a
/// shell, so detection has nothing to classify — the CLI's own help says "Use --until unknown
/// explicitly when needed"). Without it a `GOAL_RESULT`-then-exit agent sits in the until-set's
/// blind spot and every poll burns the full timeout before `check()` can classify the exit. A
/// deleted record is covered too: `herdr agent wait` then fails immediately with
/// `agent_not_found`/`agent_not_running` (verified against herdr 0.8.2), and the caller ignores
/// the exit status.
const HERDR_AGENT_STATES: [&str; 5] = ["idle", "working", "blocked", "done", "unknown"];

/// Does `wait_terminal` return promptly for this observation? `None` is the agent-gone case
/// (`herdr agent get` fails, which status.rs synthesizes as "none"): the wait errors out
/// immediately instead of matching a state. `working` is the one status that must keep waiting.
fn wait_returns_for(status: Option<&str>) -> bool {
    match status {
        None => true,
        Some(state) => HERDR_AGENT_STATES.contains(&state) && state != "working",
    }
}

/// `herdr agent wait task --until idle --until done --until blocked --until unknown --timeout 300000`
/// (server-side, event-driven — used by `status --wait`.)
pub fn wait_terminal(manifest: &Manifest, timeout_ms: u64) {
    let mut args = vec![
        HERDR.to_string(),
        "agent".to_string(),
        "wait".to_string(),
        manifest.agent_name.clone(),
    ];
    for state in HERDR_AGENT_STATES
        .into_iter()
        .filter(|state| wait_returns_for(Some(state)))
    {
        args.push("--until".to_string());
        args.push(state.to_string());
    }
    args.push("--timeout".to_string());
    args.push(timeout_ms.to_string());
    let _ = docker::exec(&manifest.container, Some("agent"), &env(), &args, false);
}

/// `herdr agent rename <pane> task` — the stable target name attach/status use. The pane exists
/// before herdr registers the agent record for it, so poll until the rename lands.
pub fn rename_to_task(manifest: &Manifest) -> anyhow::Result<()> {
    let args = vec![
        HERDR.to_string(),
        "agent".to_string(),
        "rename".to_string(),
        manifest.pane.clone(),
        "task".to_string(),
    ];
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        match docker::exec_ok(&manifest.container, Some("agent"), &env(), &args) {
            Ok(_) => return Ok(()),
            Err(err) if std::time::Instant::now() < deadline => {
                redact::eemit(&format!("rename not yet possible ({err:#}); retrying"));
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
            Err(err) => return Err(err),
        }
    }
}

/// `herdr agent prompt <target> "<text>"` — bracketed-paste + Enter; refuses on an open dialog.
pub fn prompt(manifest: &Manifest, text: &str) -> anyhow::Result<()> {
    let args = vec![
        HERDR.to_string(),
        "agent".to_string(),
        "prompt".to_string(),
        manifest.agent_name.clone(),
        text.to_string(),
    ];
    docker::exec_ok(&manifest.container, Some("agent"), &env(), &args).map(|_| ())
}

/// `herdr agent send-keys <target> enter`
pub fn send_enter(manifest: &Manifest) {
    let args = vec![
        HERDR.to_string(),
        "agent".to_string(),
        "send-keys".to_string(),
        manifest.agent_name.clone(),
        "enter".to_string(),
    ];
    let _ = docker::exec(&manifest.container, Some("agent"), &env(), &args, false);
}

/// `herdr agent get <target>` parsed for `.result.agent.agent_status`.
pub fn agent_status(manifest: &Manifest) -> Option<String> {
    let args = vec![
        HERDR.to_string(),
        "agent".to_string(),
        "get".to_string(),
        manifest.agent_name.clone(),
    ];
    let out = docker::exec(&manifest.container, Some("agent"), &env(), &args, false).ok()?;
    if !out.ok() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(out.stdout.trim()).ok()?;
    value
        .pointer("/result/agent/agent_status")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// `herdr pane read <pane> --source visible` — the last rendered screen.
/// (`--source recent*` returns empty on headless Linux; never use it.)
pub fn pane_visible(manifest: &Manifest) -> anyhow::Result<String> {
    let args = vec![
        HERDR.to_string(),
        "pane".to_string(),
        "read".to_string(),
        manifest.pane.clone(),
        "--source".to_string(),
        "visible".to_string(),
    ];
    let out = docker::exec(&manifest.container, Some("agent"), &env(), &args, false)?;
    if !out.ok() {
        anyhow::bail!(
            "herdr pane read failed: {}",
            redact::scrub(out.stderr.trim_end())
        );
    }
    Ok(out.stdout)
}

/// Snapshot the visible pane into `<run>/out/screen.txt` (best effort).
pub fn snapshot_screen(manifest: &Manifest, run_dir: &std::path::Path) {
    if let Ok(screen) = pane_visible(manifest)
        && let Err(err) =
            redact::write_scrubbed(&run_dir.join("out").join("screen.txt"), screen.as_bytes())
    {
        redact::eemit(&format!("screen snapshot failed: {err}"));
    }
}

/// `herdr status` inside the container — is the server up?
pub fn server_reachable(manifest: &Manifest) -> bool {
    let args = vec![HERDR.to_string(), "status".to_string()];
    docker::exec(&manifest.container, Some("agent"), &env(), &args, false)
        .map(|out| out.ok())
        .unwrap_or(false)
}

/// Extract `.result.root_pane.pane_id` from a `herdr workspace create` JSON line.
pub fn pane_from_workspace_create(stdout: &str) -> anyhow::Result<String> {
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).context("herdr workspace create did not print JSON")?;
    let pane = value
        .pointer("/result/root_pane/pane_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    if pane.is_empty() || pane == "null" {
        anyhow::bail!("no pane id in herdr workspace create output");
    }
    Ok(pane)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stall fix: a `GOAL_RESULT`-then-exit agent must not burn the full 300 s timeout per
    /// poll. herdr 0.8.2 reports that shape as `unknown` (process gone, nothing to classify) or
    /// drops the record entirely (`agent get` fails → `None` here); both must make the wait
    /// return immediately so `check()` can classify the exit.
    #[test]
    fn wait_returns_immediately_when_the_agent_is_gone() {
        assert!(wait_returns_for(None), "agent record gone");
        assert!(
            wait_returns_for(Some("unknown")),
            "herdr's process-gone classification"
        );
    }

    #[test]
    fn wait_returns_for_the_settle_states() {
        for state in ["idle", "done", "blocked"] {
            assert!(wait_returns_for(Some(state)), "{state}");
        }
    }

    #[test]
    fn wait_keeps_waiting_while_the_agent_works() {
        assert!(!wait_returns_for(Some("working")));
    }

    /// The until-set handed to `herdr agent wait` is derived from the predicate, so a status the
    /// predicate accepts can never be left out of the CLI call — that blind spot was the stall.
    #[test]
    fn until_set_covers_every_known_state_but_working() {
        for state in HERDR_AGENT_STATES {
            assert_eq!(wait_returns_for(Some(state)), state != "working", "{state}");
        }
    }
}
