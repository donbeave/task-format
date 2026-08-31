//! Completion signals read from outside the container (port of the `status.sh` state machine).
//!
//! Trust order: the Claude transcript `goal_status` verdict, then the agent-authored
//! `GOAL_RESULT` line — the transcript jsonl first (authoritative), the raw `tui.log` as
//! fallback — then herdr's own agent status.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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

/// Last `GOAL_RESULT` line in the raw tui log, as rendered (see [`strip_ansi`]).
pub fn goal_result_line(tui_log: &Path) -> Option<String> {
    let text = std::fs::read_to_string(tui_log).ok()?;
    text.lines()
        .rev()
        .map(strip_ansi)
        .find(|line| line.starts_with("GOAL_RESULT"))
}

/// Last `GOAL_RESULT` line in the session transcript jsonl — the authoritative copy of the
/// agent's final report. `tui.log` is a terminal *rendering* and loses rows to redraw and
/// compaction: in the 2026-08-31 TASK-002 run the agent printed
/// `GOAL_RESULT task=TASK-002 status=DONE` and the log never showed it, so the run polled IDLE
/// until `kill_after` with the work done. The transcript records the assistant message itself
/// and cannot truncate it.
///
/// A row only counts when it is anchored to *this* run — it must carry `task=<task_id>` and a
/// parseable `status=` token — because an assistant text block also quotes the line while
/// planning ("next I print GOAL_RESULT ..."), and a mid-task quote is not completion evidence.
pub fn goal_result_transcript(transcript: &Path, task_id: &str) -> Option<String> {
    let text = std::fs::read_to_string(transcript).ok()?;
    let anchor = format!("task={task_id}");
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
        for block in content {
            if block.get("type").and_then(serde_json::Value::as_str) != Some("text") {
                continue;
            }
            let Some(text) = block.get("text").and_then(serde_json::Value::as_str) else {
                continue;
            };
            for row in text.lines() {
                let row = row.trim();
                if row.starts_with("GOAL_RESULT")
                    && row.split_whitespace().any(|token| token == anchor)
                    && report_status(row).is_some()
                {
                    last = Some(row.to_string());
                }
            }
        }
    }
    last
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
    match value.trim_end_matches([',', ';']) {
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
    fn goal_result_line_is_the_last_one_and_cr_is_stripped() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("tui.log");
        std::fs::write(&log, "noise\r\nGOAL_RESULT task=TASK-101 status=BLOCKED\r\nmore noise\nGOAL_RESULT task=TASK-101 status=DONE\n").unwrap();
        assert_eq!(
            goal_result_line(&log).unwrap(),
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
            goal_result_line(&log).unwrap(),
            "GOAL_RESULT task=TASK-002 status=DONE",
            "a rendered GOAL_RESULT row must be readable; before the strip, none ever was"
        );
        assert_eq!(
            report_status(&goal_result_line(&log).unwrap()),
            Some(ReportStatus::Done)
        );
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
                {\"type\":\"text\",\"text\":\"planning mention\\nGOAL_RESULT task=T-1 status=DONE\"}]}}\n",
        );
        assert_eq!(
            goal_result_transcript(&dir.path().join("t.jsonl"), "T-1").as_deref(),
            Some("GOAL_RESULT task=T-1 status=DONE"),
            "the real final line, anchored to the run's task, counts"
        );
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
}
