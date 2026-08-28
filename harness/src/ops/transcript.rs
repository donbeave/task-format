//! Completion signals read from outside the container (port of the `status.sh` state machine).
//!
//! Trust order: the Claude transcript `goal_status` verdict, then the agent-authored
//! `GOAL_RESULT` line in the raw `tui.log`, then herdr's own agent status.

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

/// Last `GOAL_RESULT` line in the raw tui log (CR stripped).
pub fn goal_result_line(tui_log: &Path) -> Option<String> {
    let text = std::fs::read_to_string(tui_log).ok()?;
    text.lines()
        .map(|line| line.trim_end_matches('\r'))
        .rev()
        .find(|line| line.starts_with("GOAL_RESULT"))
        .map(str::to_string)
}

/// Did the goal evaluator die?
pub fn goal_cleared_error(tui_log: &Path) -> bool {
    std::fs::read_to_string(tui_log)
        .map(|text| text.contains("Goal cleared after an unrecoverable error"))
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
        let age = chrono::Utc::now().signed_duration_since(ts).to_std().ok();
        return age.is_some_and(|age| age < max_age);
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
}
