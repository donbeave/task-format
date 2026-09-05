//! `taskfmt status <RUN>` — completion detection from outside the container (port of status.sh).

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Context;
use serde::Serialize;

use crate::cmds::Ctx;
use crate::ops::{herdr, transcript};
use crate::redact;
use crate::runstate::Manifest;

pub const CODEX_TRANSCRIPT_NA: &str = "n/a (rollout jsonl not parsed)";

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub state: String,
    pub herdr_status: String,
    pub goal_reason: String,
    pub goal_result_line: String,
    /// Agent-authored `status=` of `goal_result_line` (DONE|BLOCKED|NEEDS_REPLAN|INCOMPLETE); a
    /// label only, never load-bearing.
    pub report_status: Option<String>,
    /// Real (non-sentinel) evaluator verdicts; `None` for non-claude agents (rollout jsonl is not
    /// parsed).
    pub goal_verdicts: Option<usize>,
    /// **Why** the harness considers the run finished — `goal-verdict`, `agent-exited`,
    /// `container-stopped`, `killed-timeout`, `goal-cleared-error`, `evaluator-error` — and
    /// `None` while it does not.
    /// This is the field that makes `terminal()` answerable by an operator instead of inferred
    /// from a state name whose meaning depends on which signal produced it.
    pub terminal_reason: Option<String>,
    /// Claude session transcript path, or `n/a (rollout jsonl not parsed)` for codex.
    pub transcript: String,
    /// The trusted base commit recorded at dispatch (the scope base of record).
    pub base_sha: String,
    /// Does the agent-writable `baseline` tag in the run workspace still point at `base_sha`?
    /// `None` when the workspace is absent.
    pub base_tag_ok: Option<bool>,
}

impl Status {
    /// A run is finished when there is a *reason* it is finished. `state != RUNNING` was not that
    /// test: `IDLE` is herdr's liveness heuristic and a thinking model, a cold `cargo build` and a
    /// compaction pause all render as idle, so an `IDLE` on its own says nothing about completion.
    pub fn terminal(&self) -> bool {
        self.terminal_reason.is_some()
    }

    /// Evidence that the agent actually finished, as opposed to merely going quiet: the
    /// transcript carries a real evaluator verdict, or the agent printed its final `GOAL_RESULT`
    /// report. `goal_verdicts` is `None` for codex (the rollout jsonl is not parsed), which is
    /// exactly why the second arm exists.
    pub fn completion_evidence(&self) -> bool {
        self.goal_verdicts.is_some_and(|count| count >= 1)
            || !self.goal_result_line.trim().is_empty()
    }

    /// A status with only the state and the manifest-derived fields filled in.
    fn bare(state: &str, manifest: &Manifest, run_dir: &Path) -> Self {
        Self {
            state: state.to_string(),
            herdr_status: String::new(),
            goal_reason: String::new(),
            goal_result_line: String::new(),
            report_status: None,
            goal_verdicts: None,
            // no transcript was read, so the only reasons available are the hard events
            terminal_reason: terminal_reason_for(state, false).map(str::to_string),
            transcript: transcript_display(manifest),
            base_sha: manifest.base_sha.clone(),
            base_tag_ok: base_tag_ok(manifest, run_dir),
        }
    }
}

/// Where the authoritative transcript is (claude) or why there is none (codex).
pub fn transcript_display(manifest: &Manifest) -> String {
    if manifest.agent_kind == "claude" {
        transcript::claude_transcript(manifest)
            .display()
            .to_string()
    } else {
        CODEX_TRANSCRIPT_NA.to_string()
    }
}

/// `/work` is agent-writable, so the `baseline` tag can move: compare the live tag with the SHA
/// recorded at dispatch. Diagnostic only — the scope base of record is `manifest.base_sha`.
/// `Some(false)` means the tag moved or was deleted; `None` means no answer: the run workspace is
/// not a git checkout, or git itself failed (lock, corrupt repo, spawn error), which must not be
/// reported as drift.
pub fn base_tag_ok(manifest: &Manifest, run_dir: &Path) -> Option<bool> {
    let workspace = run_dir.join("workspace");
    if !workspace.join(".git").exists() || manifest.base_sha.is_empty() {
        return None;
    }
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(&workspace).args([
        "rev-parse",
        "--verify",
        "--quiet",
        &format!("{}^{{commit}}", crate::cmds::run::BASE_TAG),
    ]);
    let out = crate::ops::capture(&mut cmd).ok()?;
    match out.status {
        0 => Some(out.stdout.trim() == manifest.base_sha),
        // `--verify --quiet` exits 1 for a ref that does not resolve: the tag is gone.
        1 => Some(false),
        _ => None,
    }
}

/// `docker stop` reached it first.
pub const CONTAINER_STOPPED: &str = "CONTAINER_STOPPED";
/// The evaluator says the goal condition is met.
pub const GOAL_MET: &str = "GOAL_MET";
/// The evaluator died.
pub const GOAL_CLEARED_ERROR: &str = "GOAL_CLEARED_ERROR";
/// The evaluator's Stop hook crashed before it ever produced a verdict.
pub const EVALUATOR_ERROR: &str = "EVALUATOR_ERROR";
/// The pane is back at the shell — no live agent.
pub const AGENT_EXITED: &str = "AGENT_EXITED";
/// herdr says the agent settled with the goal still set.
pub const IDLE: &str = "IDLE";
/// A dialog is open.
pub const BLOCKED: &str = "BLOCKED";
pub const RUNNING: &str = "RUNNING";
pub const KILLED_TIMEOUT: &str = "KILLED_TIMEOUT";

/// `Status::terminal_reason`: the transcript carries an evaluator verdict, or the agent printed
/// its final `GOAL_RESULT` report. The only reason that says the *work* reached an end.
pub const REASON_GOAL_VERDICT: &str = "goal-verdict";
/// `Status::terminal_reason`: the pane is back at a shell — there is no agent left to finish.
pub const REASON_AGENT_EXITED: &str = "agent-exited";
/// `Status::terminal_reason`: the container is gone.
pub const REASON_CONTAINER_STOPPED: &str = "container-stopped";
/// `Status::terminal_reason`: `kill_after_min` elapsed and the harness cleared the goal.
pub const REASON_KILLED_TIMEOUT: &str = "killed-timeout";
/// `Status::terminal_reason`: the goal evaluator died, so no verdict can ever arrive.
pub const REASON_GOAL_CLEARED_ERROR: &str = "goal-cleared-error";
/// `Status::terminal_reason`: the evaluator's Stop hook failed with zero verdicts on record.
/// That no verdict can arrive afterwards is a harness-protocol invariant — nothing re-prompts
/// the evaluator before `kill_after` (check-ins disabled) — not a transcript invariant.
pub const REASON_EVALUATOR_ERROR: &str = "evaluator-error";

/// `<run>/out/` — one JSON line per poll of [`wait_terminal_state`], so the latch's decisions are
/// an artifact instead of an inference. Diagnosing the 2026-08-29 TASK-002 race needed exactly
/// this file and it did not exist.
pub const STATUS_DECISIONS_LOG: &str = "status-decisions.log";

/// The terminal-state vocabulary, mapped to the reason that justifies it.
///
/// `IDLE` and `BLOCKED` are herdr's *liveness* classifications and are not completion evidence:
/// they are terminal only when the transcript or the agent's own final report says the work ended.
/// Everything else here is a hard event that no amount of further polling can change.
fn terminal_reason_for(state: &str, completion_evidence: bool) -> Option<&'static str> {
    match state {
        CONTAINER_STOPPED => Some(REASON_CONTAINER_STOPPED),
        AGENT_EXITED => Some(REASON_AGENT_EXITED),
        KILLED_TIMEOUT => Some(REASON_KILLED_TIMEOUT),
        GOAL_MET => Some(REASON_GOAL_VERDICT),
        GOAL_CLEARED_ERROR => Some(REASON_GOAL_CLEARED_ERROR),
        EVALUATOR_ERROR => Some(REASON_EVALUATOR_ERROR),
        IDLE | BLOCKED if completion_evidence => Some(REASON_GOAL_VERDICT),
        _ => None,
    }
}

pub fn run(ctx: &Ctx, run_id: &str, wait: bool, kill_after: Option<u64>) -> anyhow::Result<i32> {
    let (resolved, run_dir) = crate::cmds::load_run(ctx, run_id)?;
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
                break Status::bare(KILLED_TIMEOUT, &manifest, &run_dir);
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
pub fn check(manifest: &Manifest, run_dir: &Path) -> anyhow::Result<Status> {
    let mut state = RUNNING.to_string();
    let mut reason = String::new();
    let mut verdicts: Option<usize> = None;

    // 1. authoritative (claude): last evaluator verdict in the transcript
    let tr = transcript::claude_transcript(manifest);
    let claude = manifest.agent_kind == "claude" && tr.is_file();
    let mut evaluator_error = false;
    if claude {
        if let Some(verdict) = transcript::goal_verdict(&tr) {
            reason = verdict.reason;
            if verdict.met {
                state = GOAL_MET.to_string();
            }
        }
        verdicts = Some(transcript::verdict_count(&tr));
        if transcript::goal_cleared_error(&manifest.tui_log()) {
            state = GOAL_CLEARED_ERROR.to_string();
        }
        evaluator_error = transcript::evaluator_hook_error(&tr);
    }

    // 2. agent-side signal (both agents): the GOAL_RESULT line. The transcript jsonl is
    //    authoritative — `tui.log` is a rendering and loses the final report to redraw and
    //    compaction (the 2026-08-31 TASK-002 run) — so the log is only a fallback.
    let result = if claude {
        transcript::goal_result_transcript(&tr, &manifest.task)
            .or_else(|| transcript::goal_result_line(&manifest.tui_log()))
    } else {
        transcript::goal_result_line(&manifest.tui_log())
    }
    .unwrap_or_default();

    let evidence = verdicts.is_some_and(|count| count >= 1) || !result.trim().is_empty();

    // The evaluator's Stop hook crashed and never judged (zero verdicts). Terminal — polling
    // cannot produce a verdict anymore — but only when nothing else says the work ended:
    // completion evidence (a verdict, or the agent's verifiable GOAL_RESULT) always wins, and
    // the gate decides whether the work may be pushed.
    if evaluator_error && verdicts == Some(0) && !evidence && state == RUNNING {
        state = EVALUATOR_ERROR.to_string();
    }

    let container_running = crate::ops::docker::is_running(&manifest.container);
    let mut hstate = "none".to_string();
    if !container_running {
        // A stopped container must not mask artifact evidence: a verdict or a GOAL_RESULT read
        // from disk still classifies the run. Only an evidence-free run is CONTAINER_STOPPED.
        if state == RUNNING {
            state = if evidence {
                IDLE.to_string()
            } else {
                CONTAINER_STOPPED.to_string()
            };
        }
    } else {
        // herdr's own classification (idle|working|blocked|done|unknown|none)
        hstate = herdr::agent_status(manifest).unwrap_or_else(|| "none".to_string());
        if hstate.is_empty() {
            hstate = "none".to_string();
        }
        if hstate == "none" && state == RUNNING {
            // No live agent. Like a stopped container, this must not mask artifact evidence:
            // with a verdict or a GOAL_RESULT on disk the work verifiably ended, and the gate
            // decides — the bare agent-exited hard event is for the evidence-free case only.
            state = if evidence {
                IDLE.to_string()
            } else {
                AGENT_EXITED.to_string()
            };
        }

        // 3. herdr says settled and nothing else fired
        if state == RUNNING {
            match hstate.as_str() {
                "idle" | "done" => state = IDLE.to_string(),
                "blocked" => state = BLOCKED.to_string(),
                _ => {}
            }
        }

        // 4. herdr misreads a busy claude (spinner frames classify as idle/blocked). Fresh
        //    transcript activity is ground truth for "working right now", so settle states
        //    downgrade to RUNNING; verdicts and hard exits are never touched.
        //    With anchored completion evidence (a verdict, or a GOAL_RESULT carrying the task id
        //    and a parseable status token — the anchoring is what made the false-positive risk
        //    negligible) the agent has formally reported an end, so the quiet-window buys
        //    nothing: skip the downgrade and let the evidence classify terminal immediately.
        if matches!(state.as_str(), IDLE | BLOCKED) {
            let active = transcript::recently_active(&tr, ACTIVE_WINDOW);
            state = settle_with_activity(&state, evidence, active).to_string();
        }
    }

    // 5. the state is a label; the reason is what makes it terminal. `IDLE`/`BLOCKED` need
    //    completion evidence, and without it the caller keeps polling until `kill_after_min`.
    let terminal_reason = terminal_reason_for(&state, evidence).map(str::to_string);

    Ok(Status {
        state,
        herdr_status: hstate,
        goal_reason: reason,
        report_status: transcript::report_status(&result).map(|status| status.as_str().to_string()),
        goal_result_line: result,
        goal_verdicts: verdicts,
        terminal_reason,
        transcript: transcript_display(manifest),
        base_sha: manifest.base_sha.clone(),
        base_tag_ok: base_tag_ok(manifest, run_dir),
    })
}

/// Transcript silence that still counts as "the agent may be mid-tool" (a cold cargo build writes
/// no events until it finishes).
const ACTIVE_WINDOW: Duration = Duration::from_secs(300);

/// herdr misclassifies a busy claude: spinner frames read as idle/blocked. The transcript is the
/// ground truth for "is the agent doing anything": fresh activity downgrades those two states
/// back to RUNNING — the rescue for a genuinely busy agent going quiet in herdr's eyes.
/// Verdicts and hard exits are never downgraded, and neither is a settle state backed by
/// anchored completion evidence (a real evaluator verdict, or a GOAL_RESULT carrying the task
/// id plus a parseable status token): the agent has formally reported an end, and that
/// anchoring already removed the false-positive risk, so the quiet-window buys nothing there.
fn settle_with_activity(state: &str, evidence: bool, transcript_active: bool) -> &str {
    if evidence {
        return state;
    }
    downgrade_if_active(state, transcript_active)
}

fn downgrade_if_active(state: &str, transcript_active: bool) -> &str {
    if transcript_active && matches!(state, IDLE | BLOCKED) {
        RUNNING
    } else {
        state
    }
}

fn json_line(status: &Status) -> String {
    serde_json::to_string(status).unwrap_or_else(|_| "{{}}".to_string())
}

/// Used by `run --wait` and `experiment`: poll until terminal or the deadline.
pub fn wait_terminal_state(
    manifest: &Manifest,
    run_dir: &Path,
    deadline: Duration,
) -> anyhow::Result<Status> {
    // herdr's per-frame classification flickers, so a terminal state must hold before we gate on
    // it: an IDLE blip right after prompt injection (before the agent picks up work) and a
    // BLOCKED misread while a spinner renders must both pass unnoticed.
    const WARMUP: Duration = Duration::from_secs(90);
    const SETTLE: Duration = Duration::from_secs(30);
    let started = Instant::now();
    let mut candidate: Option<(String, Duration)> = None;
    let mut announced: Option<String> = None;
    loop {
        herdr::wait_terminal(manifest, 300_000);
        let status = check(manifest, run_dir).context("status check failed")?;
        let elapsed = started.elapsed();
        let (next, confirmed) = latch_decision(
            &candidate,
            elapsed,
            &status.state,
            status.terminal(),
            WARMUP,
            SETTLE,
        );
        candidate = next;
        log_decision(run_dir, elapsed, &status, &candidate, confirmed);
        // One line per *change*, not per poll: a run that sits in `IDLE` without a verdict for an
        // hour must say so once, or an operator watching the console sees an unexplained wait.
        if announced.as_deref() != Some(status.state.as_str()) {
            announced = Some(status.state.clone());
            if !status.terminal() && matches!(status.state.as_str(), IDLE | BLOCKED) {
                redact::eemit(&format!(
                    "{} is {} with no completion evidence (goal_verdicts={:?}, GOAL_RESULT {}) — not terminal; polling until kill_after",
                    manifest.run,
                    status.state,
                    status.goal_verdicts,
                    if status.goal_result_line.is_empty() {
                        "absent"
                    } else {
                        "present"
                    },
                ));
            }
        }
        if confirmed {
            return Ok(status);
        }
        if elapsed >= deadline {
            let _ = herdr::prompt(manifest, "/goal clear");
            let killed = Status::bare(KILLED_TIMEOUT, manifest, run_dir);
            log_decision(run_dir, elapsed, &killed, &None, true);
            return Ok(killed);
        }
        std::thread::sleep(Duration::from_secs(10));
    }
}

/// Append one poll to `<run>/out/status-decisions.log`. Best effort: a run must not fail because
/// its diary could not be written, but a run whose latch fired for a reason nobody can reconstruct
/// is how the 2026-08-29 TASK-002 race stayed invisible for an hour of analysis.
fn log_decision(
    run_dir: &Path,
    elapsed: Duration,
    status: &Status,
    candidate: &Option<(String, Duration)>,
    confirmed: bool,
) {
    let record = serde_json::json!({
        "at": crate::config::timestamp_rfc3339(),
        "elapsed_s": elapsed.as_secs(),
        "state": status.state,
        "herdr_status": status.herdr_status,
        "goal_verdicts": status.goal_verdicts,
        "goal_result_line": status.goal_result_line,
        "report_status": status.report_status,
        "terminal_reason": status.terminal_reason,
        "completion_evidence": status.completion_evidence(),
        "candidate": candidate.as_ref().map(|(state, _)| state),
        "candidate_since_s": candidate.as_ref().map(|(_, at)| at.as_secs()),
        "confirmed": confirmed,
    });
    let out_dir = run_dir.join("out");
    if std::fs::create_dir_all(&out_dir).is_err() {
        return;
    }
    let line = redact::scrub(&format!("{record}\n"));
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out_dir.join(STATUS_DECISIONS_LOG))
    {
        let _ = std::io::Write::write_all(&mut file, line.as_bytes());
    }
}

/// Stability latch over observed states. `elapsed` is time since polling began, `candidate` the
/// terminal state seen last (with the elapsed time it was first seen at). A terminal state is
/// confirmed when it has held for `settle`; a state that is not terminal — `RUNNING`, or an
/// `IDLE`/`BLOCKED` that carries no completion evidence — resets the latch; a bare IDLE inside
/// `warmup` is a pre-work blip and resets it too. Returns the next candidate and whether the
/// current state is confirmed terminal.
///
/// `terminal` is passed in rather than derived from `state` because the same state name means
/// different things depending on the evidence behind it, and that distinction is the whole fix:
/// nothing that is not terminal can ever be confirmed here, so the caller keeps polling to
/// `kill_after_min` and reaches `KILLED_TIMEOUT`, which is not promotable.
fn latch_decision(
    candidate: &Option<(String, Duration)>,
    elapsed: Duration,
    state: &str,
    terminal: bool,
    warmup: Duration,
    settle: Duration,
) -> (Option<(String, Duration)>, bool) {
    if !terminal {
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

/// A run that must not be promoted when the agent is still working — or when nothing establishes
/// that it ever stopped.
///
/// `GOAL_MET` carries an evaluator verdict by construction. `IDLE` and `GOAL_CLEARED_ERROR` do
/// not: an `IDLE` with neither a verdict nor a `GOAL_RESULT` line is the settled-idle heuristic
/// and nothing more, and a `GOAL_CLEARED_ERROR` before any verdict means the evaluator died
/// without ever judging. Neither is a run whose work may be pushed. `EVALUATOR_ERROR` is never
/// promotable by construction (it is only classified when no completion evidence exists), like
/// `KILLED_TIMEOUT`.
pub fn is_promotable(status: &Status) -> bool {
    match status.state.as_str() {
        GOAL_MET => true,
        IDLE | GOAL_CLEARED_ERROR => status.completion_evidence(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WARMUP: Duration = Duration::from_secs(90);
    const SETTLE: Duration = Duration::from_secs(30);

    /// One poll of a run that HAS completion evidence (a verdict or a `GOAL_RESULT` line).
    fn observe(
        candidate: &Option<(String, Duration)>,
        elapsed: u64,
        state: &str,
    ) -> (Option<(String, Duration)>, bool) {
        observe_with(candidate, elapsed, state, true)
    }

    /// One poll of a run with NO completion evidence: herdr's classification and nothing else.
    fn observe_bare(
        candidate: &Option<(String, Duration)>,
        elapsed: u64,
        state: &str,
    ) -> (Option<(String, Duration)>, bool) {
        observe_with(candidate, elapsed, state, false)
    }

    fn observe_with(
        candidate: &Option<(String, Duration)>,
        elapsed: u64,
        state: &str,
        evidence: bool,
    ) -> (Option<(String, Duration)>, bool) {
        latch_decision(
            candidate,
            Duration::from_secs(elapsed),
            state,
            terminal_reason_for(state, evidence).is_some(),
            WARMUP,
            SETTLE,
        )
    }

    fn status(state: &str, verdicts: Option<usize>, goal_result_line: &str) -> Status {
        let evidence =
            verdicts.is_some_and(|count| count >= 1) || !goal_result_line.trim().is_empty();
        Status {
            state: state.to_string(),
            herdr_status: "idle".into(),
            goal_reason: String::new(),
            goal_result_line: goal_result_line.to_string(),
            report_status: None,
            goal_verdicts: verdicts,
            terminal_reason: terminal_reason_for(state, evidence).map(str::to_string),
            transcript: String::new(),
            base_sha: "abc".into(),
            base_tag_ok: None,
        }
    }

    /// The decision table this file exists to get right. Left column: what the harness observes.
    /// Right column: is the run finished, and if so on what grounds.
    /// The latch keeps a diary, because reconstructing why it fired from `tui.log` and a manifest
    /// is what made the 2026-08-29 TASK-002 race cost an hour of analysis.
    #[test]
    fn every_poll_is_appended_to_the_decisions_log() {
        let dir = tempfile::tempdir().unwrap();
        let waiting = status(IDLE, Some(0), "");
        log_decision(dir.path(), Duration::from_secs(400), &waiting, &None, false);
        let done = status(IDLE, Some(1), "GOAL_RESULT task=TASK-002 status=DONE");
        log_decision(
            dir.path(),
            Duration::from_secs(440),
            &done,
            &Some((IDLE.to_string(), Duration::from_secs(410))),
            true,
        );

        let text =
            std::fs::read_to_string(dir.path().join("out").join(STATUS_DECISIONS_LOG)).unwrap();
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(
            lines.len(),
            2,
            "one JSON line per poll, appended never truncated"
        );

        assert_eq!(lines[0]["state"], IDLE);
        assert_eq!(lines[0]["elapsed_s"], 400);
        assert_eq!(lines[0]["goal_verdicts"], 0);
        assert_eq!(lines[0]["completion_evidence"], false);
        assert!(
            lines[0]["terminal_reason"].is_null(),
            "no reason, no terminal"
        );
        assert_eq!(lines[0]["confirmed"], false);
        assert!(lines[0]["candidate"].is_null());

        assert_eq!(lines[1]["terminal_reason"], REASON_GOAL_VERDICT);
        assert_eq!(lines[1]["completion_evidence"], true);
        assert_eq!(lines[1]["confirmed"], true);
        assert_eq!(lines[1]["candidate"], IDLE);
        assert_eq!(lines[1]["candidate_since_s"], 410);
        assert_eq!(
            lines[1]["goal_result_line"],
            "GOAL_RESULT task=TASK-002 status=DONE"
        );
    }

    #[test]
    fn terminal_reason_is_the_whole_decision_table() {
        // hard events: terminal whatever the transcript says
        for state in [
            CONTAINER_STOPPED,
            AGENT_EXITED,
            KILLED_TIMEOUT,
            GOAL_CLEARED_ERROR,
            EVALUATOR_ERROR,
        ] {
            assert!(terminal_reason_for(state, false).is_some(), "{state}");
        }
        assert_eq!(
            terminal_reason_for(CONTAINER_STOPPED, false),
            Some(REASON_CONTAINER_STOPPED)
        );
        assert_eq!(
            terminal_reason_for(AGENT_EXITED, false),
            Some(REASON_AGENT_EXITED)
        );
        assert_eq!(
            terminal_reason_for(KILLED_TIMEOUT, false),
            Some(REASON_KILLED_TIMEOUT)
        );
        assert_eq!(
            terminal_reason_for(GOAL_CLEARED_ERROR, false),
            Some(REASON_GOAL_CLEARED_ERROR)
        );
        assert_eq!(
            terminal_reason_for(EVALUATOR_ERROR, false),
            Some(REASON_EVALUATOR_ERROR)
        );
        // an evaluator verdict is terminal on its own
        assert_eq!(
            terminal_reason_for(GOAL_MET, false),
            Some(REASON_GOAL_VERDICT)
        );
        // herdr's liveness heuristics are terminal ONLY with completion evidence
        assert_eq!(terminal_reason_for(IDLE, false), None);
        assert_eq!(terminal_reason_for(BLOCKED, false), None);
        assert_eq!(terminal_reason_for(IDLE, true), Some(REASON_GOAL_VERDICT));
        assert_eq!(
            terminal_reason_for(BLOCKED, true),
            Some(REASON_GOAL_VERDICT)
        );
        // RUNNING is never terminal
        assert_eq!(terminal_reason_for(RUNNING, true), None);
    }

    #[test]
    fn completion_evidence_accepts_a_verdict_or_a_final_report() {
        // claude: a real (non-sentinel) evaluator verdict
        assert!(status(IDLE, Some(1), "").completion_evidence());
        assert!(!status(IDLE, Some(0), "").completion_evidence());
        // codex: no verdict count is available at all, so the agent's own report is the evidence
        assert!(status(IDLE, None, "GOAL_RESULT task=TASK-002 status=DONE").completion_evidence());
        assert!(!status(IDLE, None, "   ").completion_evidence());
        assert!(!status(IDLE, None, "").completion_evidence());
    }

    /// The 2026-08-29 TASK-002 run, replayed: herdr said `idle` while a cold `cargo test` produced
    /// 359 s of transcript silence, the goal verdict did not arrive for another three minutes, and
    /// the old latch confirmed on the settle alone and walked the gate into a moving tree.
    #[test]
    fn idle_without_a_verdict_never_confirms_however_long_it_holds() {
        let mut candidate = None;
        for elapsed in [40, 100, 200, 400, 900, 3600] {
            let (next, confirmed) = observe_bare(&candidate, elapsed, IDLE);
            candidate = next;
            assert!(
                !confirmed,
                "IDLE with no verdict must never confirm (t={elapsed}s)"
            );
            assert!(
                candidate.is_none(),
                "and must not even latch (t={elapsed}s)"
            );
        }
        // the verdict finally lands: now it settles like any other terminal state
        let (candidate, confirmed) = observe(&candidate, 3610, IDLE);
        assert!(!confirmed);
        let (_, confirmed) = observe(&candidate, 3650, IDLE);
        assert!(
            confirmed,
            "IDLE WITH a verdict is terminal after the settle"
        );
    }

    #[test]
    fn blocked_without_a_verdict_never_confirms_either() {
        let (candidate, confirmed) = observe_bare(&None, 400, BLOCKED);
        assert!(!confirmed);
        assert!(candidate.is_none());
        let (_, confirmed) = observe_bare(&candidate, 900, BLOCKED);
        assert!(!confirmed);
    }

    /// What ends a run that never produces evidence: the deadline, and nothing else.
    #[test]
    fn the_deadline_is_the_only_exit_from_an_evidence_free_idle() {
        let killed = status(KILLED_TIMEOUT, None, "");
        assert!(killed.terminal());
        assert_eq!(
            killed.terminal_reason.as_deref(),
            Some(REASON_KILLED_TIMEOUT)
        );
        assert!(!is_promotable(&killed), "a timeout is never promotable");
        assert!(!status(IDLE, Some(0), "").terminal());
        assert!(!status(BLOCKED, None, "").terminal());
        assert!(!status(RUNNING, Some(1), "").terminal());
    }

    #[test]
    fn promotable_requires_evidence_not_just_a_quiet_pane() {
        assert!(is_promotable(&status(GOAL_MET, Some(1), "")));
        assert!(is_promotable(&status(IDLE, Some(2), "")));
        assert!(is_promotable(&status(
            IDLE,
            None,
            "GOAL_RESULT task=TASK-002 status=DONE"
        )));
        // the exact shape of the 2026-08-29 TASK-002 run at gate time
        assert!(!is_promotable(&status(IDLE, Some(0), "")));
        // the evaluator died before it ever judged: nothing to promote on
        assert!(!is_promotable(&status(GOAL_CLEARED_ERROR, Some(0), "")));
        assert!(is_promotable(&status(GOAL_CLEARED_ERROR, Some(1), "")));
        // the evaluator's Stop hook crashed with zero verdicts: terminal, never promotable
        let evaluator = status(EVALUATOR_ERROR, Some(0), "");
        assert!(evaluator.terminal());
        assert_eq!(
            evaluator.terminal_reason.as_deref(),
            Some(REASON_EVALUATOR_ERROR)
        );
        assert!(!is_promotable(&evaluator));
        // ... but when the agent's GOAL_RESULT survived in the transcript, the evidence wins
        // and the gate decides
        assert!(is_promotable(&status(
            IDLE,
            Some(0),
            "GOAL_RESULT task=TASK-002 status=DONE"
        )));
        // hard exits and dialogs stay out, as before
        assert!(!is_promotable(&status(AGENT_EXITED, Some(1), "")));
        assert!(!is_promotable(&status(CONTAINER_STOPPED, Some(1), "")));
        assert!(!is_promotable(&status(BLOCKED, Some(1), "")));
        assert!(!is_promotable(&status(RUNNING, Some(1), "")));
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
    fn evaluator_error_latches_like_any_hard_terminal() {
        // terminal with zero evidence by construction; no warmup applies (that is IDLE-only),
        // and it confirms once it has held for the settle window
        let (candidate, confirmed) = observe_bare(&None, 10, EVALUATOR_ERROR);
        assert!(!confirmed, "not before the settle window");
        assert_eq!(
            candidate.as_ref().map(|(state, _)| state.as_str()),
            Some(EVALUATOR_ERROR)
        );
        let (_, confirmed) = observe_bare(&candidate, 45, EVALUATOR_ERROR);
        assert!(confirmed, "confirmed after holding through the settle");
    }

    #[test]
    fn goal_met_needs_settle_but_no_warmup() {
        // an instant GOAL_MET (trivial task) is authoritative: warmup applies to IDLE only
        let (candidate, confirmed) = observe(&None, 10, GOAL_MET);
        assert!(!confirmed);
        let (_, confirmed) = observe(&candidate, 45, GOAL_MET);
        assert!(confirmed);
    }

    /// The latency fix: completion evidence (an anchored GOAL_RESULT or a real verdict) means
    /// the agent formally reported an end, so fresh transcript activity must NOT keep the run
    /// in RUNNING for the whole 300 s quiet-window — the terminal classification is immediate
    /// (the latch settle still applies, unchanged). `check()` itself is not directly testable
    /// (docker/herdr), so the rule is exercised at the `settle_with_activity` seam.
    #[test]
    fn completion_evidence_skips_the_recent_activity_downgrade() {
        // IDLE + evidence + fresh activity: stays IDLE, stays terminal-classifiable
        assert_eq!(settle_with_activity(IDLE, true, true), IDLE);
        assert_eq!(settle_with_activity(BLOCKED, true, true), BLOCKED);
        assert_eq!(
            terminal_reason_for(settle_with_activity(IDLE, true, true), true),
            Some(REASON_GOAL_VERDICT),
            "evidence + activity must not delay terminal classification"
        );
        // IDLE + no evidence + fresh activity: downgraded to RUNNING, exactly as today
        assert_eq!(settle_with_activity(IDLE, false, true), RUNNING);
        assert_eq!(settle_with_activity(BLOCKED, false, true), RUNNING);
        // no fresh activity: evidence changes nothing
        assert_eq!(settle_with_activity(IDLE, true, false), IDLE);
        assert_eq!(settle_with_activity(IDLE, false, false), IDLE);
    }

    #[test]
    fn fresh_transcript_activity_downgrades_settle_states_only() {
        assert_eq!(downgrade_if_active(IDLE, true), RUNNING);
        assert_eq!(downgrade_if_active(BLOCKED, true), RUNNING);
        assert_eq!(downgrade_if_active(IDLE, false), IDLE);
        assert_eq!(downgrade_if_active(BLOCKED, false), BLOCKED);
        // verdicts and hard exits survive an active transcript
        assert_eq!(downgrade_if_active(GOAL_MET, true), GOAL_MET);
        assert_eq!(
            downgrade_if_active(CONTAINER_STOPPED, true),
            CONTAINER_STOPPED
        );
        assert_eq!(downgrade_if_active(AGENT_EXITED, true), AGENT_EXITED);
    }

    #[test]
    fn recently_active_follows_the_last_assistant_event() {
        let dir = tempfile::tempdir().unwrap();
        let tr = dir.path().join("t.jsonl");
        // fresh mtime but a 10-minute-old assistant event: idle, not active
        let old = (chrono::Utc::now() - chrono::Duration::seconds(600)).to_rfc3339();
        std::fs::write(
            &tr,
            format!("{{\"type\":\"assistant\",\"timestamp\":\"{old}\"}}\n"),
        )
        .unwrap();
        assert!(!transcript::recently_active(&tr, Duration::from_secs(300)));
    }

    fn manifest(kind: &str, run_dir: &Path, base_sha: &str) -> Manifest {
        Manifest {
            run: "r".into(),
            run_dir: run_dir.display().to_string(),
            container: "harness-r".into(),
            agent: "p".into(),
            agent_kind: kind.into(),
            model: String::new(),
            effort: "high".into(),
            task: "TASK-001".into(),
            repo_url: "https://example.invalid/x.git".into(),
            base_sha: base_sha.into(),
            clone_sha: String::new(),
            session_id: "sid".into(),
            pane: String::new(),
            agent_name: "task".into(),
            start: String::new(),
            selfcheck: crate::runstate::SELFCHECK_NOT_RUN.into(),
            experiment: None,
            gate: None,
            status_state: String::new(),
            result_sha: None,
            pending_promotion_sha: None,
        }
    }

    #[test]
    fn transcript_display_by_kind() {
        let dir = tempfile::tempdir().unwrap();
        let claude = manifest("claude", dir.path(), "abc");
        assert!(transcript_display(&claude).ends_with("agent-home/projects/work/sid.jsonl"));
        let codex = manifest("codex", dir.path(), "abc");
        assert_eq!(transcript_display(&codex), CODEX_TRANSCRIPT_NA);
    }

    #[test]
    fn base_tag_ok_is_none_without_a_workspace_and_tracks_tag_drift() {
        let dir = tempfile::tempdir().unwrap();
        let m = manifest("claude", dir.path(), "abc");
        assert_eq!(base_tag_ok(&m, dir.path()), None);

        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .current_dir(&workspace)
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        git(&["init", "-q"]);
        git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@x",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "base",
        ]);
        let base = git(&["rev-parse", "HEAD"]);
        git(&["tag", "baseline"]);
        let m = manifest("claude", dir.path(), &base);
        assert_eq!(base_tag_ok(&m, dir.path()), Some(true));
        git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@x",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "moved",
        ]);
        git(&["tag", "-f", "baseline"]);
        assert_eq!(base_tag_ok(&m, dir.path()), Some(false));
        git(&["tag", "-d", "baseline"]);
        assert_eq!(
            base_tag_ok(&m, dir.path()),
            Some(false),
            "a deleted tag is drift, not a missing answer"
        );
        let bare = Status::bare(CONTAINER_STOPPED, &m, dir.path());
        assert_eq!(bare.goal_verdicts, None);
        assert_eq!(bare.base_sha, base);
        assert_eq!(
            bare.terminal_reason.as_deref(),
            Some(REASON_CONTAINER_STOPPED)
        );
        assert!(bare.terminal());
        let json: serde_json::Value = serde_json::from_str(&json_line(&bare)).unwrap();
        assert_eq!(json["terminal_reason"], "container-stopped");
        assert!(json["goal_verdicts"].is_null());
        assert_eq!(json["base_tag_ok"], serde_json::Value::Bool(false));
        assert!(json["report_status"].is_null());
        git(&["tag", "baseline", &base]);
        assert_eq!(base_tag_ok(&m, dir.path()), Some(true));
    }

    #[test]
    fn base_tag_ok_is_none_when_git_cannot_answer() {
        // `.git` exists but is not a repository: git exits 128, which is no verdict on the tag.
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(workspace.join(".git")).unwrap();
        let m = manifest("claude", dir.path(), "abc");
        assert_eq!(base_tag_ok(&m, dir.path()), None);
    }
}
