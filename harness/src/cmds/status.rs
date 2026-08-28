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
    // herdr's per-frame classification flickers, so a terminal state must hold before we gate on
    // it: an IDLE blip right after prompt injection (before the agent picks up work) and a
    // BLOCKED misread while a spinner renders must both pass unnoticed.
    const WARMUP: Duration = Duration::from_secs(90);
    const SETTLE: Duration = Duration::from_secs(30);
    let started = Instant::now();
    let mut candidate: Option<(String, Duration)> = None;
    loop {
        herdr::wait_terminal(manifest, 300_000);
        let status = check(manifest, run_dir).context("status check failed")?;
        let elapsed = started.elapsed();
        let (next, confirmed) = latch_decision(&candidate, elapsed, &status.state, WARMUP, SETTLE);
        candidate = next;
        if confirmed {
            return Ok(status);
        }
        if elapsed >= deadline {
            let _ = herdr::prompt(manifest, "/goal clear");
            return Ok(Status {
                state: KILLED_TIMEOUT.to_string(),
                herdr_status: String::new(),
                goal_reason: String::new(),
                goal_result_line: String::new(),
                goal_verdicts: 0,
            });
        }
        std::thread::sleep(Duration::from_secs(10));
    }
}

/// Stability latch over observed states. `elapsed` is time since polling began, `candidate` the
/// terminal state seen last (with the elapsed time it was first seen at). A terminal state is
/// confirmed when it has held for `settle`; a bare IDLE inside `warmup` is a pre-work blip and
/// resets the latch; anything RUNNING resets it too. Returns the next candidate and whether the
/// current state is confirmed terminal.
fn latch_decision(
    candidate: &Option<(String, Duration)>,
    elapsed: Duration,
    state: &str,
    warmup: Duration,
    settle: Duration,
) -> (Option<(String, Duration)>, bool) {
    if state == RUNNING {
        return (None, false);
    }
    if state == IDLE && elapsed < warmup {
        return (None, false);
    }
    match candidate {
        Some((seen, at)) if seen == state => {
            let confirmed = elapsed.saturating_sub(*at) >= settle;
            (candidate.clone(), confirmed)
        }
        _ => (Some((state.to_string(), elapsed)), false),
    }
}

/// A run that must not be promoted when the agent is still working.
pub fn is_promotable(status: &Status) -> bool {
    matches!(status.state.as_str(), GOAL_MET | IDLE | GOAL_CLEARED_ERROR)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WARMUP: Duration = Duration::from_secs(90);
    const SETTLE: Duration = Duration::from_secs(30);

    fn observe(
        candidate: &Option<(String, Duration)>,
        elapsed: u64,
        state: &str,
    ) -> (Option<(String, Duration)>, bool) {
        latch_decision(
            candidate,
            Duration::from_secs(elapsed),
            state,
            WARMUP,
            SETTLE,
        )
    }

    #[test]
    fn idle_blip_inside_warmup_never_confirms() {
        // prompt injected at t=0, herdr reports idle at t=40 (agent has not picked up work yet)
        let (candidate, confirmed) = observe(&None, 40, IDLE);
        assert!(!confirmed);
        let (_, confirmed) = observe(&candidate, 80, IDLE);
        assert!(
            !confirmed,
            "an idle still inside warmup must stay unconfirmed"
        );
    }

    #[test]
    fn idle_after_warmup_confirms_once_it_holds() {
        let (candidate, confirmed) = observe(&None, 200, IDLE);
        assert!(!confirmed);
        // the same state, re-observed a full settle later
        let (_, confirmed) = observe(&candidate, 240, IDLE);
        assert!(confirmed);
    }

    #[test]
    fn working_resets_the_latch_and_a_new_state_starts_over() {
        let (candidate, _) = observe(&None, 200, IDLE);
        let (candidate, confirmed) = observe(&candidate, 210, RUNNING);
        assert!(!confirmed);
        assert!(candidate.is_none());
        // blocked appears, but only for one poll: a spinner misread, not a dialog
        let (candidate, confirmed) = observe(&candidate, 220, BLOCKED);
        assert!(!confirmed);
        let (candidate, confirmed) = observe(&candidate, 260, RUNNING);
        assert!(!confirmed);
        assert!(candidate.is_none());
        let _ = candidate;
    }

    #[test]
    fn goal_met_needs_settle_but_no_warmup() {
        // an instant GOAL_MET (trivial task) is authoritative: warmup applies to IDLE only
        let (candidate, confirmed) = observe(&None, 10, GOAL_MET);
        assert!(!confirmed);
        let (_, confirmed) = observe(&candidate, 45, GOAL_MET);
        assert!(confirmed);
    }
}
