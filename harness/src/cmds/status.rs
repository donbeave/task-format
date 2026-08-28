//! `taskfmt status <RUN>` — completion detection from outside the container (port of status.sh).

use std::time::{Duration, Instant};

use anyhow::Context;
use serde::Serialize;

use crate::cmds::Ctx;
use crate::ops::{herdr, transcript};
use crate::redact;
use crate::runstate::Manifest;

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub state: String,
    pub herdr_status: String,
    pub goal_reason: String,
    pub goal_result_line: String,
    pub goal_verdicts: usize,
}

impl Status {
    pub fn terminal(&self) -> bool {
        self.state != "RUNNING"
    }
}

/// `docker stop` reached it first.
pub const CONTAINER_STOPPED: &str = "CONTAINER_STOPPED";
/// The evaluator says the goal condition is met.
pub const GOAL_MET: &str = "GOAL_MET";
/// The evaluator died.
pub const GOAL_CLEARED_ERROR: &str = "GOAL_CLEARED_ERROR";
/// The pane is back at the shell — no live agent.
pub const AGENT_EXITED: &str = "AGENT_EXITED";
/// herdr says the agent settled with the goal still set.
pub const IDLE: &str = "IDLE";
/// A dialog is open.
pub const BLOCKED: &str = "BLOCKED";
pub const RUNNING: &str = "RUNNING";
pub const KILLED_TIMEOUT: &str = "KILLED_TIMEOUT";

pub fn run(ctx: &Ctx, run_id: &str, wait: bool, kill_after: Option<u64>) -> anyhow::Result<i32> {
    let resolved = ctx.load()?;
    let run_dir = resolved.run_dir(run_id);
    let manifest = Manifest::load(&run_dir)?;

    let status = if wait {
        let kill_after =
            Duration::from_secs(60 * kill_after.unwrap_or(resolved.cfg.runtime.kill_after_min));
        let started = Instant::now();
        loop {
            herdr::wait_terminal(&manifest, 300_000);
            let status = check(&manifest, &run_dir)?;
            redact::emit(&json_line(&status));
            if status.terminal() {
                break status;
            }
            if started.elapsed() >= kill_after {
                // `/goal clear` stops the loop; the operator inspects the live container after.
                let _ = herdr::prompt(&manifest, "/goal clear");
                redact::emit(&format!("{{\"state\":\"{KILLED_TIMEOUT}\"}}"));
                break Status {
                    state: KILLED_TIMEOUT.to_string(),
                    herdr_status: String::new(),
                    goal_reason: String::new(),
                    goal_result_line: String::new(),
                    goal_verdicts: 0,
                };
            }
        }
    } else {
        let status = check(&manifest, &run_dir)?;
        redact::emit(&json_line(&status));
        status
    };

    herdr::snapshot_screen(&manifest, &run_dir);
    Ok(if status.terminal() { 0 } else { 3 })
}

/// The three-signal state machine.
pub fn check(manifest: &Manifest, _run_dir: &std::path::Path) -> anyhow::Result<Status> {
    let mut state = RUNNING.to_string();
    let mut reason = String::new();
    let mut verdicts = 0usize;

    if !crate::ops::docker::is_running(&manifest.container) {
        return Ok(Status {
            state: CONTAINER_STOPPED.to_string(),
            herdr_status: String::new(),
            goal_reason: String::new(),
            goal_result_line: String::new(),
            goal_verdicts: 0,
        });
    }

    // herdr's own classification (idle|working|blocked|done|unknown|none)
    let mut hstate = herdr::agent_status(manifest).unwrap_or_else(|| "none".to_string());
    if hstate.is_empty() {
        hstate = "none".to_string();
    }
    if hstate == "none" {
        state = AGENT_EXITED.to_string();
    }

    // 1. authoritative (claude): last evaluator verdict in the transcript
    let tr = transcript::claude_transcript(manifest);
    if manifest.agent_kind == "claude" && tr.is_file() {
        if let Some(verdict) = transcript::goal_verdict(&tr) {
            reason = verdict.reason;
            if verdict.met {
                state = GOAL_MET.to_string();
            }
        }
        verdicts = transcript::verdict_count(&tr);
        if transcript::goal_cleared_error(&manifest.tui_log()) {
            state = GOAL_CLEARED_ERROR.to_string();
        }
    }

    // 2. agent-side signal (both agents): the GOAL_RESULT line in the raw log
    let result = transcript::goal_result_line(&manifest.tui_log()).unwrap_or_default();

    // 3. herdr says settled and nothing else fired
    if state == RUNNING {
        match hstate.as_str() {
            "idle" | "done" => state = IDLE.to_string(),
            "blocked" => state = BLOCKED.to_string(),
            _ => {}
        }
    }

    Ok(Status {
        state,
        herdr_status: hstate,
        goal_reason: reason,
        goal_result_line: result,
        goal_verdicts: verdicts,
    })
}

fn json_line(status: &Status) -> String {
    serde_json::to_string(status).unwrap_or_else(|_| "{{}}".to_string())
}

/// Used by `run --wait` and `experiment`: poll until terminal or the deadline.
pub fn wait_terminal_state(
    manifest: &Manifest,
    run_dir: &std::path::Path,
    deadline: Duration,
) -> anyhow::Result<Status> {
    let started = Instant::now();
    loop {
        herdr::wait_terminal(manifest, 300_000);
        let status = check(manifest, run_dir).context("status check failed")?;
        if status.terminal() {
            return Ok(status);
        }
        if started.elapsed() >= deadline {
            let _ = herdr::prompt(manifest, "/goal clear");
            return Ok(Status {
                state: KILLED_TIMEOUT.to_string(),
                herdr_status: String::new(),
                goal_reason: String::new(),
                goal_result_line: String::new(),
                goal_verdicts: 0,
            });
        }
    }
}

/// A run that must not be promoted when the agent is still working.
pub fn is_promotable(status: &Status) -> bool {
    matches!(status.state.as_str(), GOAL_MET | IDLE | GOAL_CLEARED_ERROR)
}
