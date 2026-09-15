//! Completion signals read from outside the container (port of the `status.sh` state machine).
//!
//! Trust order: the Claude transcript `goal_status` verdict, then the agent-authored
//! `GOAL_RESULT` line — the transcript jsonl first (authoritative), the raw `tui.log` as
//! fallback — then herdr's own agent status.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::runstate::Manifest;

/// A transcript `goal_status` attachment (non-sentinel = a real evaluator verdict).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalVerdict {
    pub met: bool,
    pub reason: String,
}

/// `<agent-home>/projects/work/<session>.jsonl` (`CLAUDE_CODE_PROJECT_DIR_NAME=work`).
pub fn claude_transcript(manifest: &Manifest) -> PathBuf {
    manifest
        .run_dir_path()
        .join("agent-home")
        .join("projects")
        .join("work")
        .join(format!("{}.jsonl", manifest.session_id))
}

/// `<agent-home>/sessions/**/rollout-*.jsonl` — Codex's durable session log.
pub fn codex_sessions_dir(manifest: &Manifest) -> PathBuf {
    manifest.run_dir_path().join("agent-home").join("sessions")
}

/// Newest matching Codex rollout under [`codex_sessions_dir`], preferring this run's
/// `session_id` in the filename or the first-line `session_meta.id`.
pub fn codex_rollout(manifest: &Manifest) -> Option<PathBuf> {
    codex_rollout_in(&codex_sessions_dir(manifest), &manifest.session_id)
}

fn codex_rollout_in(sessions: &Path, session_id: &str) -> Option<PathBuf> {
    if !sessions.is_dir() {
        return None;
    }
    let mut matches = Vec::new();
    for entry in WalkDir::new(sessions)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with("rollout-") && name.ends_with(".jsonl") {
            matches.push(path.to_path_buf());
        }
    }
    if matches.is_empty() {
        return None;
    }
    if let Some(found) = matches
        .iter()
        .find(|path| path.to_string_lossy().contains(session_id))
    {
        return Some(found.clone());
    }
    if let Some(found) = matches
        .iter()
        .find(|path| rollout_session_id(path).is_some_and(|id| id == session_id))
    {
        return Some(found.clone());
    }
    matches.sort_by_key(|path| {
        std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH)
    });
    matches.pop()
}

fn rollout_session_id(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let line = text.lines().next()?;
    let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
    if value.get("type")?.as_str()? != "session_meta" {
        return None;
    }
    value
        .pointer("/payload/id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// Last non-sentinel `goal_status` verdict in the transcript.
pub fn goal_verdict(transcript: &Path) -> Option<GoalVerdict> {
    let text = std::fs::read_to_string(transcript).ok()?;
    let mut last: Option<GoalVerdict> = None;
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("type").and_then(serde_json::Value::as_str) != Some("attachment") {
            continue;
        }
        let Some(attachment) = value.get("attachment") else {
            continue;
        };
        if attachment.get("type").and_then(serde_json::Value::as_str) != Some("goal_status") {
            continue;
        }
        if attachment
            .get("sentinel")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        {
            continue;
        }
        last = Some(GoalVerdict {
            met: attachment
                .get("met")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            reason: attachment
                .get("reason")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        });
    }
    last
}

/// How many real (non-sentinel) verdicts the transcript holds.
pub fn verdict_count(transcript: &Path) -> usize {
    let Some(text) = std::fs::read_to_string(transcript).ok() else {
        return 0;
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|value| {
            value.get("type").and_then(serde_json::Value::as_str) == Some("attachment")
                && value
                    .pointer("/attachment/type")
                    .and_then(serde_json::Value::as_str)
                    == Some("goal_status")
                && value
                    .pointer("/attachment/sentinel")
                    .and_then(serde_json::Value::as_bool)
                    != Some(true)
        })
        .count()
}

/// Flatten one line of a raw `script(1)` capture into the text a human sees on that row.
///
/// `tui.log` is a terminal *rendering*, not a text stream. The agent's final report reaches it as
/// `ESC[5G GOAL_RESULT ESC[17G task=TASK-002 ESC[31G status=DONE` — the cursor-column escapes are
/// what paint the spaces between the tokens, so deleting them outright would weld the tokens
/// together and a plain `starts_with("GOAL_RESULT")` on the raw line never matches at all. Each
/// escape sequence and each other control byte therefore becomes a single space, and runs of
/// whitespace collapse, which reproduces the rendered row well enough to match and to tokenize.
///
/// Measured on `experiments/runs/20260829-194052-zai-flash-TASK-002/out/tui.log`: four lines
/// contain `GOAL_RESULT` and **none of them begins with it**, so before this the harness could not
/// see an agent's final report in any real run.
pub fn strip_ansi(line: &str) -> String {
    let bytes: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i];
        if ch == '\u{1b}' {
            i += 1;
            match bytes.get(i) {
                // CSI: parameters/intermediates, then one final byte in 0x40..=0x7e
                Some('[') => {
                    i += 1;
                    while i < bytes.len() && !matches!(bytes[i], '\u{40}'..='\u{7e}') {
                        i += 1;
                    }
                    i += 1;
                }
                // OSC: runs to BEL or to ST (ESC \)
                Some(']') => {
                    i += 1;
                    while i < bytes.len() && bytes[i] != '\u{7}' {
                        if bytes[i] == '\u{1b}' && bytes.get(i + 1) == Some(&'\\') {
                            i += 1;
                            break;
                        }
                        i += 1;
                    }
                    i += 1;
                }
                // any other two-byte escape
                Some(_) => i += 1,
                None => {}
            }
            out.push(' ');
            continue;
        }
        if ch.is_control() {
            out.push(' ');
        } else {
            out.push(ch);
        }
        i += 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Why a `GOAL_RESULT` candidate is visible but not run-owned completion evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum GoalResultReject {
    WrongTask { found: String, expected: String },
    InvalidStatus,
}

impl GoalResultReject {
    /// Operator-facing fragment for `wait_terminal_state` wait messages.
    pub fn diagnostic(&self) -> String {
        match self {
            Self::WrongTask { found, expected } => {
                format!("rejected(wrong-task {found}, expected {expected})")
            }
            Self::InvalidStatus => "rejected(invalid status)".to_string(),
        }
    }
}

/// Last rendered row in `tui.log` that begins with `GOAL_RESULT`, regardless of task ownership.
pub fn last_goal_result_candidate(tui_log: &Path) -> Option<String> {
    let text = std::fs::read_to_string(tui_log).ok()?;
    text.lines()
        .rev()
        .map(strip_ansi)
        .find(|line| line.starts_with("GOAL_RESULT"))
}

/// Last assistant-text row in the transcript jsonl that begins with `GOAL_RESULT`, regardless of
/// task ownership. Same final-row rule as [`goal_result_transcript`], but without anchoring.
pub fn last_goal_result_candidate_transcript(transcript: &Path) -> Option<String> {
    let text = std::fs::read_to_string(transcript).ok()?;
    let mut last = None;
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("type").and_then(serde_json::Value::as_str) != Some("assistant") {
            continue;
        }
        let Some(content) = value
            .pointer("/message/content")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        let mut final_text_row = None;
        for block in content {
            if block.get("type").and_then(serde_json::Value::as_str) != Some("text") {
                continue;
            }
            let Some(text) = block.get("text").and_then(serde_json::Value::as_str) else {
                continue;
            };
            for row in text.lines() {
                let row = row.trim();
                if !row.is_empty() {
                    final_text_row = Some(row);
                }
            }
        }
        if let Some(row) = final_text_row
            && row.starts_with("GOAL_RESULT")
        {
            last = Some(row.to_string());
        }
    }
    last
}

/// Classify a `GOAL_RESULT` candidate against this run's task id. `None` when the line is valid
/// run-owned evidence; `Some` when the marker is present but fails task anchoring or protocol shape.
pub fn classify_goal_result_reject(line: &str, task_id: &str) -> Option<GoalResultReject> {
    if parse_goal_result(line, task_id).is_some() {
        return None;
    }
    let mut tokens = line.split_whitespace();
    if tokens.next()? != "GOAL_RESULT" {
        return None;
    }
    let task_token = tokens.next()?;
    let Some(found_task) = task_token.strip_prefix("task=") else {
        return Some(GoalResultReject::InvalidStatus);
    };
    if found_task != task_id {
        return Some(GoalResultReject::WrongTask {
            found: found_task.to_string(),
            expected: task_id.to_string(),
        });
    }
    Some(GoalResultReject::InvalidStatus)
}

/// Last valid, run-owned `GOAL_RESULT` line in the raw tui log, as rendered (see [`strip_ansi`]).
///
/// `tui.log` contains prompts, documentation, command output, and agent text. A line beginning
/// with `GOAL_RESULT` is therefore only a candidate; it becomes evidence only when its task id
/// matches this run and its status is one of the protocol values.
pub fn goal_result_line(tui_log: &Path, task_id: &str) -> Option<String> {
    last_goal_result_candidate(tui_log).filter(|line| parse_goal_result(line, task_id).is_some())
}

/// Last `GOAL_RESULT` line in the session transcript jsonl — the authoritative copy of the
/// agent's final report. `tui.log` is a terminal *rendering* and loses rows to redraw and
/// compaction: in the 2026-08-31 TASK-002 run the agent printed
/// `GOAL_RESULT task=TASK-002 status=DONE` and the log never showed it, so the run polled IDLE
/// until `kill_after` with the work done. The transcript records the assistant message itself
/// and cannot truncate it.
///
/// A report only counts when its final non-empty assistant-text row is anchored to *this* run. A
/// mid-task quote followed by more assistant text is not completion evidence.
pub fn goal_result_transcript(transcript: &Path, task_id: &str) -> Option<String> {
    let text = std::fs::read_to_string(transcript).ok()?;
    let mut last: Option<String> = None;
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("type").and_then(serde_json::Value::as_str) != Some("assistant") {
            continue;
        }
        let Some(content) = value
            .pointer("/message/content")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        let mut final_text_row = None;
        for block in content {
            if block.get("type").and_then(serde_json::Value::as_str) != Some("text") {
                continue;
            }
            let Some(text) = block.get("text").and_then(serde_json::Value::as_str) else {
                continue;
            };
            for row in text.lines() {
                let row = row.trim();
                if !row.is_empty() {
                    final_text_row = Some(row);
                }
            }
        }
        if let Some(row) = final_text_row {
            last = parse_goal_result(row, task_id)
                .is_some()
                .then(|| row.to_string());
        }
    }
    last
}

/// Last assistant-text row in a Codex rollout that begins with `GOAL_RESULT`, regardless of task
/// ownership. Same final-row rule as [`goal_result_codex_rollout`], but without anchoring.
pub fn last_goal_result_candidate_codex_rollout(rollout: &Path) -> Option<String> {
    let text = std::fs::read_to_string(rollout).ok()?;
    let mut last = None;
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let Some(row) = codex_assistant_final_row(&value)
            && row.starts_with("GOAL_RESULT")
        {
            last = Some(row.to_string());
        }
    }
    last
}

/// Last `GOAL_RESULT` line in a Codex rollout jsonl — authoritative over `tui.log` for codex
/// runs. Assistant text lives in `response_item` messages and `event_msg` `agent_message` rows.
pub fn goal_result_codex_rollout(rollout: &Path, task_id: &str) -> Option<String> {
    let text = std::fs::read_to_string(rollout).ok()?;
    let mut last = None;
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let Some(row) = codex_assistant_final_row(&value) {
            last = parse_goal_result(&row, task_id).is_some().then_some(row);
        }
    }
    last
}

fn codex_assistant_final_row(value: &serde_json::Value) -> Option<String> {
    match value.get("type").and_then(serde_json::Value::as_str)? {
        "response_item" => {
            let payload = value.get("payload")?;
            if payload.get("type").and_then(serde_json::Value::as_str)? != "message" {
                return None;
            }
            if payload.get("role").and_then(serde_json::Value::as_str)? != "assistant" {
                return None;
            }
            let content = payload
                .get("content")
                .and_then(serde_json::Value::as_array)?;
            let mut merged = String::new();
            for block in content {
                if block.get("type").and_then(serde_json::Value::as_str)? != "output_text" {
                    continue;
                }
                let Some(text) = block.get("text").and_then(serde_json::Value::as_str) else {
                    continue;
                };
                if !merged.is_empty() {
                    merged.push('\n');
                }
                merged.push_str(text);
            }
            final_text_row(&merged)
        }
        "event_msg" => {
            let payload = value.get("payload")?;
            if payload.get("type").and_then(serde_json::Value::as_str)? != "agent_message" {
                return None;
            }
            if let Some(text) = payload.get("text").and_then(serde_json::Value::as_str) {
                return final_text_row(text);
            }
            let content = payload
                .get("content")
                .and_then(serde_json::Value::as_array)?;
            let mut merged = String::new();
            for block in content {
                if block.get("type").and_then(serde_json::Value::as_str)? != "output_text" {
                    continue;
                }
                let Some(text) = block.get("text").and_then(serde_json::Value::as_str) else {
                    continue;
                };
                if !merged.is_empty() {
                    merged.push('\n');
                }
                merged.push_str(text);
            }
            final_text_row(&merged)
        }
        _ => None,
    }
}

fn final_text_row(text: &str) -> Option<String> {
    text.lines()
        .rfind(|row| !row.trim().is_empty())
        .map(|row| row.trim().to_string())
}

/// Codex TUI may show "Goal achieved" when its native goal completes. That banner is not harness
/// completion evidence — this detects it in the raw `tui.log` capture.
pub fn codex_native_goal_achieved(tui_log: &Path) -> bool {
    std::fs::read_to_string(tui_log)
        .map(|text| {
            text.lines()
                .any(|line| strip_ansi(line).contains("Goal achieved"))
        })
        .unwrap_or(false)
}

/// Did the goal evaluator's Stop hook crash? A `hook_non_blocking_error` attachment for the
/// `Stop` event means Claude Code ran the evaluator and the evaluator failed (non-zero exit,
/// unparseable output), so no `goal_status` verdict came out of that evaluation. Observed in
/// the 2026-08-31 TASK-002 run: `{"type":"hook_non_blocking_error","hookName":"Stop",
/// "hookEvent":"Stop","stderr":"JSON validation failed","exitCode":1}` — and under the harness
/// protocol no verdict can arrive afterwards either, because nothing re-prompts the evaluator
/// before `kill_after` (check-ins disabled).
pub fn evaluator_hook_error(transcript: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(transcript) else {
        return false;
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .any(|value| {
            value.get("type").and_then(serde_json::Value::as_str) == Some("attachment")
                && value
                    .pointer("/attachment/type")
                    .and_then(serde_json::Value::as_str)
                    == Some("hook_non_blocking_error")
                && value
                    .pointer("/attachment/hookEvent")
                    .and_then(serde_json::Value::as_str)
                    == Some("Stop")
        })
}

/// The agent-authored `STATUS:` in a `GOAL_RESULT` line (`status=<X>` token).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportStatus {
    Done,
    Blocked,
    NeedsReplan,
    Incomplete,
}

/// Parse the canonical, run-owned final report marker.
///
/// This deliberately accepts the protocol's exact three-token record only. The marker is emitted
/// in human-readable agent output, so accepting arbitrary fields or a placeholder would turn a
/// quoted example from `/task/AGENTS.md` into completion evidence.
pub fn parse_goal_result(line: &str, task_id: &str) -> Option<ReportStatus> {
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("GOAL_RESULT") {
        return None;
    }
    if tokens.next().and_then(|token| token.strip_prefix("task=")) != Some(task_id) {
        return None;
    }
    let status = tokens.next()?.strip_prefix("status=")?;
    if tokens.next().is_some() {
        return None;
    }
    parse_report_status(status)
}

impl ReportStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Done => "DONE",
            Self::Blocked => "BLOCKED",
            Self::NeedsReplan => "NEEDS_REPLAN",
            Self::Incomplete => "INCOMPLETE",
        }
    }
}

/// Parse the `status=` token of a `GOAL_RESULT` line; `None` when absent or not one of the four
/// protocol values (the agent's report is never load-bearing — this only labels it).
pub fn report_status(goal_result_line: &str) -> Option<ReportStatus> {
    let value = goal_result_line
        .split_whitespace()
        .find_map(|token| token.strip_prefix("status="))?;
    parse_report_status(value.trim_end_matches([',', ';']))
}

fn parse_report_status(value: &str) -> Option<ReportStatus> {
    match value {
        "DONE" => Some(ReportStatus::Done),
        "BLOCKED" => Some(ReportStatus::Blocked),
        "NEEDS_REPLAN" => Some(ReportStatus::NeedsReplan),
        "INCOMPLETE" => Some(ReportStatus::Incomplete),
        _ => None,
    }
}

/// Did the goal evaluator die? Matched on the rendered row for [`strip_ansi`]'s reason — the
/// phrase is painted by the TUI and carries the same column escapes the `GOAL_RESULT` line does.
pub fn goal_cleared_error(tui_log: &Path) -> bool {
    std::fs::read_to_string(tui_log)
        .map(|text| {
            text.lines()
                .any(|line| strip_ansi(line).contains("Goal cleared after an unrecoverable error"))
        })
        .unwrap_or(false)
}

/// The last `goal_status` sentinel (`met` flag) line — used to confirm prompt acceptance.
pub fn has_any_goal_status(transcript: &Path) -> bool {
    std::fs::read_to_string(transcript)
        .map(|text| text.contains("\"goal_status\""))
        .unwrap_or(false)
}

/// True when the last assistant event is newer than `max_age` — ground truth for "the agent is
/// doing something right now". Not file mtime: claude keeps touching the file with heartbeat
/// entries (token reminders) long after the agent settled, which would keep an idle run labeled
/// active forever.
pub fn recently_active(transcript: &Path, max_age: std::time::Duration) -> bool {
    let Ok(text) = std::fs::read_to_string(transcript) else {
        return false;
    };
    for line in text.lines().rev() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("type").and_then(serde_json::Value::as_str) != Some("assistant") {
            continue;
        }
        let Some(ts) = value
            .get("timestamp")
            .and_then(serde_json::Value::as_str)
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        else {
            continue;
        };
        // Signed on purpose. A container clock ahead of the host makes `age` negative, and the
        // earlier `to_std().ok()` turned that into `None` — i.e. "not active" — so a skew of one
        // second made every event of every run read as silence. An event stamped in the future is
        // the freshest evidence there is: treat it as "just now".
        let age = chrono::Utc::now().signed_duration_since(ts);
        let Ok(max) = chrono::Duration::from_std(max_age) else {
            return true;
        };
        return age < max;
    }
    false
}

/// True when a raw TUI capture was updated recently. For Codex, rollout jsonl under
/// `agent-home/sessions/` is the authoritative transcript, but its mtime may lag the live TUI;
/// this mtime is still the conservative activity signal to rescue a false herdr `idle`
/// classification. A future mtime is active by the same clock-skew rule as [`recently_active`].
pub fn recently_updated(path: &Path, max_age: std::time::Duration) -> bool {
    let Ok(modified) = std::fs::metadata(path).and_then(|meta| meta.modified()) else {
        return false;
    };
    let age = match std::time::SystemTime::now().duration_since(modified) {
        Ok(age) => age,
        Err(_) => return true,
    };
    age < max_age
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn write_tmp(content: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("t.jsonl"), content).unwrap();
        dir
    }

    #[test]
    fn last_non_sentinel_verdict_wins() {
        let dir = write_tmp(
            "{\"type\":\"attachment\",\"attachment\":{\"type\":\"goal_status\",\"sentinel\":true,\"met\":false}}\n\
             {\"type\":\"attachment\",\"attachment\":{\"type\":\"goal_status\",\"met\":false,\"reason\":\"not yet\"}}\n\
             {\"type\":\"other\"}\n\
             {\"type\":\"attachment\",\"attachment\":{\"type\":\"goal_status\",\"met\":true,\"reason\":\"done\"}}\n",
        );
        let verdict = goal_verdict(&dir.path().join("t.jsonl")).unwrap();
        assert!(verdict.met);
        assert_eq!(verdict.reason, "done");
        assert_eq!(verdict_count(&dir.path().join("t.jsonl")), 2);
    }

    #[test]
    fn missing_or_empty_transcript_is_not_a_verdict() {
        let dir = write_tmp("");
        assert!(goal_verdict(&dir.path().join("t.jsonl")).is_none());
        assert!(goal_verdict(dir.path().join("nope.jsonl").as_path()).is_none());
    }

    #[test]
    fn recently_active_reads_the_last_assistant_event_not_the_file_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let tr = dir.path().join("t.jsonl");
        // a fresh-looking file whose only assistant event is 10 minutes old: idle
        let old = (chrono::Utc::now() - chrono::Duration::seconds(600)).to_rfc3339();
        std::fs::write(
            &tr,
            format!(
                "{{\"type\":\"assistant\",\"timestamp\":\"{old}\"}}\n{{\"type\":\"system\"}}\n"
            ),
        )
        .unwrap();
        assert!(!recently_active(&tr, Duration::from_secs(300)));
        assert!(recently_active(&tr, Duration::from_secs(700)));

        // a recent assistant event: active, even with noise after it
        let now = chrono::Utc::now().to_rfc3339();
        std::fs::write(
            &tr,
            format!(
                "{{\"type\":\"assistant\",\"timestamp\":\"{now}\"}}\n{{\"type\":\"attachment\"}}\n"
            ),
        )
        .unwrap();
        assert!(recently_active(&tr, Duration::from_secs(300)));

        // no assistant events at all: never "active"
        std::fs::write(&tr, "{\"type\":\"system\"}\n").unwrap();
        assert!(!recently_active(&tr, Duration::from_secs(300)));
        // missing file: never active
        assert!(!recently_active(
            dir.path().join("nope.jsonl").as_path(),
            Duration::from_secs(1)
        ));
    }

    #[test]
    fn goal_result_line_is_the_last_valid_one_and_cr_is_stripped() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("tui.log");
        std::fs::write(&log, "noise\r\nGOAL_RESULT task=TASK-101 status=BLOCKED\r\nmore noise\nGOAL_RESULT task=TASK-101 status=DONE\n").unwrap();
        assert_eq!(
            goal_result_line(&log, "TASK-101").unwrap(),
            "GOAL_RESULT task=TASK-101 status=DONE"
        );
        assert!(!goal_cleared_error(&log));
        std::fs::write(&log, "Goal cleared after an unrecoverable error\n").unwrap();
        assert!(goal_cleared_error(&log));
    }

    #[test]
    fn strip_ansi_renders_a_row_instead_of_deleting_its_separators() {
        // CSI cursor-column moves are the spaces: deleting them welds the tokens together.
        assert_eq!(
            strip_ansi("\u{1b}[5GGOAL_RESULT\u{1b}[17Gtask=TASK-002\u{1b}[31Gstatus=DONE\r\r"),
            "GOAL_RESULT task=TASK-002 status=DONE"
        );
        // SGR colours, OSC titles and lone escapes all reduce to whitespace
        assert_eq!(
            strip_ansi("\u{1b}[38;2;1;2;3mred\u{1b}[39m\u{1b}]0;title\u{7}tail\u{1b}Xz"),
            "red tail z"
        );
        assert_eq!(strip_ansi("plain"), "plain");
        assert_eq!(strip_ansi(""), "");
        // a truncated escape at end of line must not panic or loop
        assert_eq!(strip_ansi("a\u{1b}[38;2"), "a");
        assert_eq!(strip_ansi("a\u{1b}"), "a");
    }

    #[test]
    fn goal_result_line_survives_the_terminal_rendering() {
        // the exact shape the last row of a real tui.log carries
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("tui.log");
        std::fs::write(
            &log,
            "\u{1b}[2C the goal prompt echoes the words GOAL_RESULT line has been printed\r\n\
             \u{1b}[5GGOAL_RESULT\u{1b}[17Gtask=TASK-002\u{1b}[31Gstatus=DONE\r\r\n",
        )
        .unwrap();
        assert_eq!(
            goal_result_line(&log, "TASK-002").unwrap(),
            "GOAL_RESULT task=TASK-002 status=DONE",
            "a rendered GOAL_RESULT row must be readable; before the strip, none ever was"
        );
        assert_eq!(
            report_status(&goal_result_line(&log, "TASK-002").unwrap()),
            Some(ReportStatus::Done)
        );
    }

    #[test]
    fn raw_goal_result_rejects_prompt_examples_and_other_tasks() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("tui.log");
        std::fs::write(
            &log,
            "GOAL_RESULT task=TASK-000 status=<STATUS>\n\
             GOAL_RESULT task=TASK-003 status=DONE\n\
             GOAL_RESULT task=TASK-004 status=DONE extra=field\n",
        )
        .unwrap();
        assert!(
            goal_result_line(&log, "TASK-004").is_none(),
            "placeholder, wrong-task, and non-canonical records are not evidence"
        );

        std::fs::write(&log, "GOAL_RESULT task=TASK-004 status=DONE\n").unwrap();
        assert_eq!(
            goal_result_line(&log, "TASK-004").as_deref(),
            Some("GOAL_RESULT task=TASK-004 status=DONE")
        );
        assert!(goal_result_line(&log, "TASK-003").is_none());
    }

    #[test]
    fn goal_cleared_error_is_matched_on_the_rendered_row() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("tui.log");
        std::fs::write(
            &log,
            "\u{1b}[5GGoal\u{1b}[10Gcleared after an unrecoverable error\r\n",
        )
        .unwrap();
        assert!(goal_cleared_error(&log));
    }

    #[test]
    fn a_timestamp_ahead_of_the_host_clock_is_active_not_silent() {
        // container clock 30 s ahead of the host: a negative age used to read as "not active",
        // which made every event of every run look like silence.
        let dir = tempfile::tempdir().unwrap();
        let tr = dir.path().join("t.jsonl");
        let future = (chrono::Utc::now() + chrono::Duration::seconds(30)).to_rfc3339();
        std::fs::write(
            &tr,
            format!("{{\"type\":\"assistant\",\"timestamp\":\"{future}\"}}\n"),
        )
        .unwrap();
        assert!(recently_active(&tr, Duration::from_secs(300)));
    }

    #[test]
    fn recently_updated_accepts_a_fresh_capture_and_rejects_an_old_one() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("tui.log");
        std::fs::write(&log, "screen").unwrap();
        assert!(recently_updated(&log, Duration::from_secs(300)));
        assert!(!recently_updated(&log, Duration::ZERO));
        assert!(!recently_updated(
            &dir.path().join("missing"),
            Duration::from_secs(300)
        ));
    }

    #[test]
    fn goal_result_transcript_reads_the_last_report_from_assistant_text() {
        let dir = write_tmp(
            "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[\
                {\"type\":\"thinking\",\"thinking\":\"GOAL_RESULT task=T-1 status=DONE is not a report\"},\
                {\"type\":\"text\",\"text\":\"STATUS: BLOCKED\\nGOAL_RESULT task=T-1 status=BLOCKED\"}]}}\n\
             {\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"GOAL_RESULT task=T-1 status=DONE\"}}\n\
             {\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[\
                {\"type\":\"text\",\"text\":\"final report\\nGOAL_RESULT task=T-1 status=DONE\"}]}}\n",
        );
        assert_eq!(
            goal_result_transcript(&dir.path().join("t.jsonl"), "T-1").as_deref(),
            Some("GOAL_RESULT task=T-1 status=DONE"),
            "assistant text blocks only, last one wins; thinking blocks and user echoes do not count"
        );
    }

    #[test]
    fn goal_result_transcript_must_be_anchored_to_the_run_task() {
        // a mid-task quote/planning mention of the line — a different task id, no task id, or no
        // parseable status — is not this run's completion evidence
        let dir = write_tmp(
            "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[\
                {\"type\":\"text\",\"text\":\"GOAL_RESULT task=T-999 status=DONE\"}]}}\n\
             {\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[\
                {\"type\":\"text\",\"text\":\"GOAL_RESULT status=DONE\"}]}}\n\
             {\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[\
                {\"type\":\"text\",\"text\":\"GOAL_RESULT task=T-1 status=MAYBE\"}]}}\n",
        );
        assert!(goal_result_transcript(&dir.path().join("t.jsonl"), "T-1").is_none());
        let dir = write_tmp(
            "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[\
                {\"type\":\"text\",\"text\":\"planning mention\\nGOAL_RESULT task=T-1 status=DONE\\nthen more work\"}]}}\n",
        );
        assert!(
            goal_result_transcript(&dir.path().join("t.jsonl"), "T-1").is_none(),
            "a marker followed by more assistant text is not a final report"
        );
    }

    fn write_codex_rollout(dir: &Path, session_id: &str, body: &str) -> PathBuf {
        let rollout_dir = dir.join("agent-home/sessions/2026/09/16");
        std::fs::create_dir_all(&rollout_dir).unwrap();
        let path = rollout_dir.join(format!("rollout-2026-09-16T00-00-00-{session_id}.jsonl"));
        let meta = format!(
            "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\"}}}}\n{body}"
        );
        std::fs::write(&path, meta).unwrap();
        path
    }

    #[test]
    fn codex_rollout_prefers_the_run_session_id() {
        let dir = tempfile::tempdir().unwrap();
        write_codex_rollout(dir.path(), "sid-a", "");
        write_codex_rollout(dir.path(), "sid-b", "");
        assert!(
            codex_rollout_in(&dir.path().join("agent-home/sessions"), "sid-b")
                .unwrap()
                .to_string_lossy()
                .contains("sid-b")
        );
    }

    #[test]
    fn goal_result_codex_rollout_reads_the_last_assistant_message() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = write_codex_rollout(
            dir.path(),
            "sid-1",
            "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[\
                {\"type\":\"output_text\",\"text\":\"STATUS: BLOCKED\\nGOAL_RESULT task=T-1 status=BLOCKED\"}]}}\n\
             {\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"text\":\"final report\\nGOAL_RESULT task=T-1 status=DONE\"}}\n",
        );
        assert_eq!(
            goal_result_codex_rollout(&rollout, "T-1").as_deref(),
            Some("GOAL_RESULT task=T-1 status=DONE"),
            "response_item and event_msg assistant rows; last anchored report wins"
        );
    }

    #[test]
    fn goal_result_codex_rollout_must_be_anchored_to_the_run_task() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = write_codex_rollout(
            dir.path(),
            "sid-1",
            "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[\
                {\"type\":\"output_text\",\"text\":\"GOAL_RESULT task=T-999 status=DONE\"}]}}\n\
             {\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[\
                {\"type\":\"output_text\",\"text\":\"planning mention\\nGOAL_RESULT task=T-1 status=DONE\\nthen more work\"}]}}\n",
        );
        assert!(goal_result_codex_rollout(&rollout, "T-1").is_none());
    }

    #[test]
    fn codex_native_goal_achieved_matches_the_rendered_banner() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("tui.log");
        std::fs::write(
            &log,
            "\u{1b}[5GGoal\u{1b}[10Gachieved\u{1b}[20Gfor this task\r\n",
        )
        .unwrap();
        assert!(codex_native_goal_achieved(&log));
        std::fs::write(&log, "still working\n").unwrap();
        assert!(!codex_native_goal_achieved(&log));
    }

    #[test]
    fn goal_result_transcript_is_none_without_a_report() {
        let dir = write_tmp(
            "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[\
                {\"type\":\"text\",\"text\":\"still working\"}]}}\n",
        );
        assert!(goal_result_transcript(&dir.path().join("t.jsonl"), "T-1").is_none());
        assert!(goal_result_transcript(dir.path().join("nope.jsonl").as_path(), "T-1").is_none());
    }

    #[test]
    fn evaluator_hook_error_matches_the_stop_hook_crash_only() {
        let dir = write_tmp(
            "{\"type\":\"attachment\",\"attachment\":{\"type\":\"hook_non_blocking_error\",\
             \"hookName\":\"Stop\",\"hookEvent\":\"Stop\",\"stderr\":\"JSON validation failed\",\"exitCode\":1}}\n",
        );
        assert!(evaluator_hook_error(&dir.path().join("t.jsonl")));
        // a non-Stop hook failure is not the evaluator
        let dir = write_tmp(
            "{\"type\":\"attachment\",\"attachment\":{\"type\":\"hook_non_blocking_error\",\
             \"hookName\":\"PreToolUse\",\"hookEvent\":\"PreToolUse\",\"exitCode\":1}}\n",
        );
        assert!(!evaluator_hook_error(&dir.path().join("t.jsonl")));
        assert!(!evaluator_hook_error(
            dir.path().join("nope.jsonl").as_path()
        ));
    }

    #[test]
    fn report_status_parses_the_four_protocol_values_only() {
        assert_eq!(
            report_status("GOAL_RESULT task=TASK-101 status=DONE"),
            Some(ReportStatus::Done)
        );
        assert_eq!(
            report_status("GOAL_RESULT status=BLOCKED reason=P-001"),
            Some(ReportStatus::Blocked)
        );
        assert_eq!(
            report_status("GOAL_RESULT task=TASK-101 status=NEEDS_REPLAN"),
            Some(ReportStatus::NeedsReplan)
        );
        assert_eq!(
            report_status("GOAL_RESULT task=TASK-101 status=INCOMPLETE turns=40"),
            Some(ReportStatus::Incomplete)
        );
        assert_eq!(
            report_status("GOAL_RESULT task=TASK-101 status=MAYBE"),
            None
        );
        assert_eq!(report_status("GOAL_RESULT task=TASK-101"), None);
        assert_eq!(report_status(""), None);
        assert_eq!(ReportStatus::NeedsReplan.as_str(), "NEEDS_REPLAN");
    }

    #[test]
    fn last_goal_result_candidate_finds_the_last_row_regardless_of_task() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("tui.log");
        std::fs::write(
            &log,
            "noise\nGOAL_RESULT task=TASK-000 status=<STATUS>\n\
             GOAL_RESULT task=TASK-003 status=DONE\n",
        )
        .unwrap();
        assert_eq!(
            last_goal_result_candidate(&log).as_deref(),
            Some("GOAL_RESULT task=TASK-003 status=DONE")
        );
        assert!(last_goal_result_candidate(dir.path().join("missing").as_path()).is_none());
    }

    #[test]
    fn classify_goal_result_reject_distinguishes_absent_wrong_task_and_bad_status() {
        assert_eq!(
            classify_goal_result_reject("GOAL_RESULT task=TASK-002 status=DONE", "TASK-002"),
            None,
            "valid run-owned evidence is not a reject"
        );
        assert_eq!(
            classify_goal_result_reject("GOAL_RESULT task=TASK-003 status=DONE", "TASK-002"),
            Some(GoalResultReject::WrongTask {
                found: "TASK-003".into(),
                expected: "TASK-002".into(),
            })
        );
        for line in [
            "GOAL_RESULT task=TASK-002 status=MAYBE",
            "GOAL_RESULT task=TASK-002 status=<STATUS>",
            "GOAL_RESULT task=TASK-002 status=DONE extra=field",
            "GOAL_RESULT status=DONE",
        ] {
            assert_eq!(
                classify_goal_result_reject(line, "TASK-002"),
                Some(GoalResultReject::InvalidStatus),
                "{line}"
            );
        }
        assert_eq!(
            GoalResultReject::WrongTask {
                found: "TASK-003".into(),
                expected: "TASK-002".into(),
            }
            .diagnostic(),
            "rejected(wrong-task TASK-003, expected TASK-002)"
        );
        assert_eq!(
            GoalResultReject::InvalidStatus.diagnostic(),
            "rejected(invalid status)"
        );
    }

    #[test]
    fn last_goal_result_candidate_transcript_tracks_unanchored_rows() {
        let dir = write_tmp(
            "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[\
                {\"type\":\"text\",\"text\":\"GOAL_RESULT task=T-999 status=DONE\"}]}}\n\
             {\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":[\
                {\"type\":\"text\",\"text\":\"final\\nGOAL_RESULT task=T-1 status=MAYBE\"}]}}\n",
        );
        assert_eq!(
            last_goal_result_candidate_transcript(&dir.path().join("t.jsonl")).as_deref(),
            Some("GOAL_RESULT task=T-1 status=MAYBE")
        );
    }

    #[test]
    fn parse_goal_result_requires_the_exact_task_and_canonical_shape() {
        for (line, expected) in [
            (
                "GOAL_RESULT task=TASK-101 status=DONE",
                Some(ReportStatus::Done),
            ),
            (
                "GOAL_RESULT task=TASK-101 status=BLOCKED",
                Some(ReportStatus::Blocked),
            ),
            (
                "GOAL_RESULT task=TASK-101 status=NEEDS_REPLAN",
                Some(ReportStatus::NeedsReplan),
            ),
            (
                "GOAL_RESULT task=TASK-101 status=INCOMPLETE",
                Some(ReportStatus::Incomplete),
            ),
        ] {
            assert_eq!(parse_goal_result(line, "TASK-101"), expected, "{line}");
        }

        for line in [
            "GOAL_RESULT task=TASK-000 status=<STATUS>",
            "GOAL_RESULT task=TASK-999 status=DONE",
            "GOAL_RESULT status=DONE",
            "GOAL_RESULT task=TASK-101",
            "GOAL_RESULT task=TASK-101 status=MAYBE",
            "GOAL_RESULT task=TASK-101 status=DONE extra=field",
            "GOAL_RESULTING task=TASK-101 status=DONE",
        ] {
            assert_eq!(parse_goal_result(line, "TASK-101"), None, "{line}");
        }
    }
}
