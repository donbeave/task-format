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
    /// Claude session transcript path, or `n/a (rollout jsonl not parsed)` for codex.
    pub transcript: String,
    /// The trusted base commit recorded at dispatch (the scope base of record).
    pub base_sha: String,
    /// Does the agent-writable `baseline` tag in the run workspace still point at `base_sha`?
    /// `None` when the workspace is absent.
    pub base_tag_ok: Option<bool>,
}

impl Status {
    pub fn terminal(&self) -> bool {
        self.state != "RUNNING"
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
    let run_dir = crate::cmds::resolve_run_arg(&resolved, run_id)?;
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

    if !crate::ops::docker::is_running(&manifest.container) {
        return Ok(Status::bare(CONTAINER_STOPPED, manifest, run_dir));
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
        verdicts = Some(transcript::verdict_count(&tr));
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

    // 4. herdr misreads a busy claude (spinner frames classify as idle/blocked). Fresh transcript
    //    activity is ground truth for "working right now", so settle states downgrade to RUNNING;
    //    verdicts and hard exits are never touched.
    if matches!(state.as_str(), IDLE | BLOCKED) {
        let active = transcript::recently_active(&tr, ACTIVE_WINDOW);
        state = downgrade_if_active(&state, active).to_string();
    }

    Ok(Status {
        state,
        herdr_status: hstate,
        goal_reason: reason,
        report_status: transcript::report_status(&result).map(|status| status.as_str().to_string()),
        goal_result_line: result,
        goal_verdicts: verdicts,
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
/// back to RUNNING. Verdicts and hard exits are never downgraded.
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
            return Ok(Status::bare(KILLED_TIMEOUT, manifest, run_dir));
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
        let json: serde_json::Value = serde_json::from_str(&json_line(&bare)).unwrap();
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
