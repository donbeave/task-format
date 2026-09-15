//! herdr — the only terminal multiplexer allowed here (hard rule). All calls run inside the run
//! container as user `agent` with `HERDR_SESSION=agent`, so the socket resolves.

use std::time::Duration;

use anyhow::Context;

use crate::runstate::Manifest;
use taskfmt::redact;

use super::docker;

const HERDR: &str = "herdr";
/// herdr stores session sockets under `$HOME/.config/herdr`. `agent-launch` runs via gosu and
/// always uses the passwd home (`/home/agent`). Cursor sets container `HOME=/agent-home` for
/// config/auth paths, which `docker exec` would inherit — pin `$HOME` here so host-side herdr
/// calls reach the same socket the in-container server opened.
const AGENT_HOME: &str = "/home/agent";

fn env() -> Vec<(String, String)> {
    vec![
        ("HERDR_SESSION".to_string(), "agent".to_string()),
        ("HOME".to_string(), AGENT_HOME.to_string()),
    ]
}

const HERDR_CONTROL_TIMEOUT: Duration = Duration::from_secs(15);

fn herdr_exec(
    manifest: &Manifest,
    args: &[String],
    timeout: Duration,
) -> anyhow::Result<taskfmt::ops::Captured> {
    docker::exec_with_timeout(
        &manifest.container,
        Some("agent"),
        &env(),
        args,
        false,
        timeout,
    )
}

fn herdr_exec_ok(
    manifest: &Manifest,
    args: &[String],
    timeout: Duration,
) -> anyhow::Result<taskfmt::ops::Captured> {
    let out = herdr_exec(manifest, args, timeout)?;
    if !out.ok() {
        anyhow::bail!(
            "herdr {} failed (rc={}): {}",
            args.get(1).map(String::as_str).unwrap_or("command"),
            out.status,
            out.stderr.trim()
        );
    }
    Ok(out)
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
    let wait = Duration::from_millis(timeout_ms).saturating_add(Duration::from_secs(30));
    let out = herdr_exec(manifest, &args, wait)?;
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
    let wait = Duration::from_millis(timeout_ms).saturating_add(Duration::from_secs(30));
    let _ = herdr_exec(manifest, &args, wait);
}

/// herdr errors that mean the agent record is not ready yet — safe to retry rename/get.
pub(crate) fn is_transient_agent_error(err: &anyhow::Error) -> bool {
    is_transient_agent_message(&err.to_string())
}

fn is_transient_agent_message(msg: &str) -> bool {
    msg.contains("agent_not_found")
        || msg.contains("agent_not_running")
        || msg.contains("server_not_running")
}

/// `herdr agent get <pane>` — has herdr registered the process on this pane yet?
fn agent_get_pane(manifest: &Manifest) -> anyhow::Result<()> {
    let args = vec![
        HERDR.to_string(),
        "agent".to_string(),
        "get".to_string(),
        manifest.pane.clone(),
    ];
    let out = herdr_exec(manifest, &args, HERDR_CONTROL_TIMEOUT)?;
    if out.ok() {
        return Ok(());
    }
    let msg = format!(
        "herdr agent get {} failed (rc={}): {}",
        manifest.pane,
        out.status,
        out.stderr.trim()
    );
    if is_transient_agent_message(&msg) {
        anyhow::bail!("{msg}")
    }
    anyhow::bail!("{msg}")
}

/// `herdr agent rename <pane> task` — the stable target name attach/status use. The pane exists
/// before herdr registers the agent record for it, so wait for `agent get` then rename; retry only
/// on `agent_not_found` / `agent_not_running` with exponential backoff.
pub fn rename_to_task(manifest: &Manifest) -> anyhow::Result<()> {
    let rename_args = vec![
        HERDR.to_string(),
        "agent".to_string(),
        "rename".to_string(),
        manifest.pane.clone(),
        "task".to_string(),
    ];
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let mut backoff = std::time::Duration::from_millis(200);
    loop {
        if std::time::Instant::now() >= deadline {
            anyhow::bail!("herdr agent rename timed out after 120 s");
        }
        match agent_get_pane(manifest) {
            Ok(()) => match herdr_exec_ok(manifest, &rename_args, HERDR_CONTROL_TIMEOUT) {
                Ok(_) => return Ok(()),
                Err(err) if is_transient_agent_error(&err) => {
                    redact::eemit(&format!("rename not yet possible ({err:#}); retrying"));
                }
                Err(err) => return Err(err),
            },
            Err(err) if is_transient_agent_error(&err) => {
                redact::eemit(&format!("agent not registered yet ({err:#}); retrying"));
            }
            Err(err) => return Err(err),
        }
        std::thread::sleep(backoff);
        backoff = (backoff * 2).min(std::time::Duration::from_secs(5));
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
    herdr_exec_ok(manifest, &args, HERDR_CONTROL_TIMEOUT).map(|_| ())
}

/// Dispatch-aware goal prompt. Cursor must receive `/goal` as typed keystrokes — bracketed-paste of
/// the full line leaves a `[Pasted text]` chip and never arms the native goal.
pub fn inject_goal_prompt(manifest: &Manifest, text: &str) -> anyhow::Result<()> {
    if manifest.agent_kind == "cursor" {
        prompt_cursor_goal(manifest, text)
    } else {
        prompt(manifest, text)
    }
}

/// Fallback inject: type `/goal ` then bracketed-paste the full prompt body. Primary cursor
/// dispatch passes the prompt on the agent argv at launch instead (see `cursor_agent_cmd`).
pub fn prompt_cursor_goal(manifest: &Manifest, text: &str) -> anyhow::Result<()> {
    const PREFIX: &str = "/goal ";
    let body = text
        .strip_prefix(PREFIX)
        .with_context(|| format!("cursor goal prompt must start with `{PREFIX}`"))?;
    send_keys_text(manifest, PREFIX)?;
    std::thread::sleep(Duration::from_millis(500));
    prompt(manifest, body)
}

/// Submit a slash command by typing it (not bracketed-paste). Used for `/goal clear` on Cursor.
pub fn prompt_slash(manifest: &Manifest, command: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        command.starts_with('/'),
        "expected slash command, got: {command:?}"
    );
    send_keys_text(manifest, command)?;
    send_enter(manifest);
    Ok(())
}

/// Clear an active `/goal` loop. Cursor needs typed slash input; other agents accept paste.
pub fn clear_goal(manifest: &Manifest) {
    let result = if manifest.agent_kind == "cursor" {
        prompt_slash(manifest, "/goal clear")
    } else {
        prompt(manifest, "/goal clear")
    };
    if let Err(err) = result {
        redact::eemit(&format!("could not clear the goal: {err:#}"));
    }
}

/// `herdr agent send-keys <target> <KEY>…` — literal keystrokes, not bracketed paste.
pub fn send_keys_text(manifest: &Manifest, text: &str) -> anyhow::Result<()> {
    let keys = send_key_tokens(text)?;
    if keys.is_empty() {
        return Ok(());
    }
    let mut args = vec![
        HERDR.to_string(),
        "agent".to_string(),
        "send-keys".to_string(),
        manifest.agent_name.clone(),
    ];
    args.extend(keys);
    herdr_exec_ok(manifest, &args, HERDR_CONTROL_TIMEOUT).map(|_| ())
}

fn send_key_tokens(text: &str) -> anyhow::Result<Vec<String>> {
    let mut keys = Vec::new();
    for ch in text.chars() {
        keys.extend(char_to_send_keys(ch).with_context(|| {
            format!("cannot send key for character {ch:?} in slash command {text:?}")
        })?);
    }
    Ok(keys)
}

fn char_to_send_keys(ch: char) -> Option<Vec<String>> {
    match ch {
        'a'..='z' | '0'..='9' => Some(vec![ch.to_string()]),
        'A'..='Z' => Some(vec![
            "shift".to_string(),
            ch.to_ascii_lowercase().to_string(),
        ]),
        ' ' => Some(vec!["space".to_string()]),
        '/' => Some(vec!["slash".to_string()]),
        '-' => Some(vec!["minus".to_string()]),
        '.' => Some(vec!["period".to_string()]),
        ',' => Some(vec!["comma".to_string()]),
        '`' => Some(vec!["backtick".to_string()]),
        '\'' => Some(vec!["quote".to_string()]),
        _ => None,
    }
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
    let _ = herdr_exec(manifest, &args, HERDR_CONTROL_TIMEOUT);
}

/// `herdr agent get <target>` parsed for `.result.agent.agent_status`.
pub fn agent_status(manifest: &Manifest) -> Option<String> {
    let args = vec![
        HERDR.to_string(),
        "agent".to_string(),
        "get".to_string(),
        manifest.agent_name.clone(),
    ];
    let out = herdr_exec(manifest, &args, HERDR_CONTROL_TIMEOUT).ok()?;
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
    let out = herdr_exec(manifest, &args, HERDR_CONTROL_TIMEOUT)?;
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
    herdr_exec(manifest, &args, HERDR_CONTROL_TIMEOUT)
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

    #[test]
    fn transient_agent_errors_are_retryable() {
        assert!(is_transient_agent_message("agent_not_found: w1:p1"));
        assert!(is_transient_agent_message("agent_not_running"));
        assert!(is_transient_agent_message("server_not_running"));
        assert!(!is_transient_agent_message("permission denied"));
    }

    #[test]
    fn docker_exec_env_pins_agent_home_for_herdr() {
        let env = super::env();
        assert_eq!(
            env.iter()
                .find(|(key, _)| key == "HOME")
                .map(|(_, value)| value.as_str()),
            Some(super::AGENT_HOME)
        );
    }

    #[test]
    fn send_key_tokens_types_goal_prefix_for_cursor() {
        assert_eq!(
            send_key_tokens("/goal ").unwrap(),
            vec![
                "slash".to_string(),
                "g".to_string(),
                "o".to_string(),
                "a".to_string(),
                "l".to_string(),
                "space".to_string(),
            ]
        );
        assert_eq!(
            send_key_tokens("/goal clear").unwrap(),
            vec![
                "slash", "g", "o", "a", "l", "space", "c", "l", "e", "a", "r",
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
        );
    }
}
