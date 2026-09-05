//! Strict, derived `progress/v1` coordination state.
//!
//! Progress records leaf transitions only. The completion gate supplies completion evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, bail, ensure};

use crate::lint;
use crate::taskfile::{self, TaskFile};

pub const SCHEMA: &str = "progress/v1";

#[derive(Debug, Clone)]
pub struct GeneratedProgress {
    pub task: String,
    pub first_leaf: String,
    pub body: String,
}

/// Generate the only valid initial event stream.
pub fn generate(task_dir: &Path) -> anyhow::Result<GeneratedProgress> {
    let readme = task_dir.join("README.md");
    let text = std::fs::read_to_string(&readme)
        .with_context(|| format!("cannot read {}", readme.display()))?;
    let findings = lint::lint_text(&text, &readme);
    let errors = findings
        .iter()
        .filter(|f| f.severity == lint::Severity::Error)
        .count();
    if errors != 0 {
        bail!("progress-init: README.md failed task-lint ({errors} error(s)); not generating");
    }
    let task = TaskFile::parse(text, &readme)?;
    let leaves = checklist_leaves(&task)?;
    let first_leaf = leaves[0].clone();
    let body = format!(
        "---\nschema: {SCHEMA}\ntask: {}\nstate: IN_PROGRESS\ncurrent: {first_leaf}\nlatest_event: 1\n---\n\n## Events\n- 1 | STARTED | {first_leaf}\n\n## Handoff\nCURRENT_FAILURE: none\nDECISIONS: none\n",
        task.id()
    );
    Ok(GeneratedProgress {
        task: task.id().to_string(),
        first_leaf,
        body,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    InProgress,
    Done,
    Blocked,
    NeedsReplan,
}
impl State {
    fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "IN_PROGRESS" => Ok(Self::InProgress),
            "DONE" => Ok(Self::Done),
            "BLOCKED" => Ok(Self::Blocked),
            "NEEDS_REPLAN" => Ok(Self::NeedsReplan),
            _ => bail!("progress: unknown state `{value}`"),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "IN_PROGRESS",
            Self::Done => "DONE",
            Self::Blocked => "BLOCKED",
            Self::NeedsReplan => "NEEDS_REPLAN",
        }
    }
}

#[cfg(test)]
mod exhaustive_tests {
    use super::*;

    fn task() -> TaskFile {
        TaskFile::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/example/README.md"))
            .unwrap()
    }

    fn text(events: &str, state: &str, current: &str, latest: u64) -> String {
        format!(
            "---\nschema: progress/v1\ntask: TASK-042\nstate: {state}\ncurrent: {current}\nlatest_event: {latest}\n---\n\n## Events\n{events}\n\n## Handoff\nCURRENT_FAILURE: none\n"
        )
    }

    #[test]
    fn rejects_header_and_section_grammar() {
        let initial = text("- 1 | STARTED | 1.1", "IN_PROGRESS", "1.1", 1);
        let cases = [
            (
                "opening",
                initial.replacen("---", "xxx", 1),
                "opening header fence",
            ),
            (
                "closing",
                initial.replacen("\n---\n\n## Events", "\n###\n\n## Events", 1),
                "missing closing header fence",
            ),
            (
                "header syntax",
                initial.replacen("state: IN_PROGRESS", "state=IN_PROGRESS", 1),
                "malformed header",
            ),
            (
                "unknown header",
                initial.replacen("state: IN_PROGRESS", "extra: nope\nstate: IN_PROGRESS", 1),
                "unknown header",
            ),
            (
                "duplicate header",
                initial.replacen(
                    "state: IN_PROGRESS",
                    "state: IN_PROGRESS\nstate: IN_PROGRESS",
                    1,
                ),
                "duplicate header",
            ),
            (
                "missing header",
                initial.replacen("current: 1.1\n", "", 1),
                "missing header `current`",
            ),
            (
                "bad schema",
                initial.replacen("progress/v1", "progress/v0", 1),
                "expected schema",
            ),
            (
                "bad task",
                initial.replacen("TASK-042", "TASK-999", 1),
                "does not match",
            ),
            (
                "bad state",
                initial.replacen("IN_PROGRESS", "WAITING", 1),
                "unknown state",
            ),
            (
                "zero latest",
                initial.replacen("latest_event: 1", "latest_event: 0", 1),
                "latest_event must be positive",
            ),
            (
                "non-numeric latest",
                initial.replacen("latest_event: 1", "latest_event: one", 1),
                "latest_event must be a positive integer",
            ),
            (
                "header separator",
                initial.replacen("---\n\n## Events", "---\n## Events", 1),
                "expected one blank line",
            ),
            (
                "events heading",
                initial.replacen("## Events", "## Event", 1),
                "missing or misplaced `## Events`",
            ),
            (
                "handoff separator",
                initial.replacen("\n\n## Handoff", "\n## Handoff", 1),
                "malformed event row",
            ),
            (
                "handoff heading",
                initial.replacen("## Handoff", "## Hand Off", 1),
                "missing or misplaced `## Handoff`",
            ),
        ];
        for (name, candidate, want) in cases {
            let error = ProgressFile::parse(&candidate, &task())
                .unwrap_err()
                .to_string();
            assert!(error.contains(want), "{name}: {error}");
        }
    }

    #[test]
    fn rejects_event_grammar_and_every_invalid_transition() {
        let cases = [
            (
                "empty events",
                text("", "IN_PROGRESS", "1.1", 1),
                "events must not be empty",
            ),
            (
                "row prefix",
                text("* 1 | STARTED | 1.1", "IN_PROGRESS", "1.1", 1),
                "malformed event row",
            ),
            (
                "row fields",
                text("- 1 | STARTED", "IN_PROGRESS", "1.1", 1),
                "malformed event row",
            ),
            (
                "unknown status",
                text("- 1 | WAITING | 1.1", "IN_PROGRESS", "1.1", 1),
                "unknown event status",
            ),
            (
                "zero sequence",
                text("- 0 | STARTED | 1.1", "IN_PROGRESS", "1.1", 1),
                "invalid event sequence",
            ),
            (
                "gapped sequence",
                text("- 2 | STARTED | 1.1", "IN_PROGRESS", "1.1", 2),
                "must start at 1 and be contiguous",
            ),
            (
                "unknown leaf",
                text("- 1 | STARTED | 9.9", "IN_PROGRESS", "9.9", 1),
                "unknown checklist leaf",
            ),
            (
                "started active",
                text(
                    "- 1 | STARTED | 1.1\n- 2 | STARTED | 2.1",
                    "IN_PROGRESS",
                    "2.1",
                    2,
                ),
                "STARTED while",
            ),
            (
                "done inactive",
                text("- 1 | DONE | 1.1", "DONE", "NONE", 1),
                "DONE requires active",
            ),
            (
                "failed inactive",
                text("- 1 | FAILED | 1.1", "IN_PROGRESS", "NONE", 1),
                "FAILED requires active",
            ),
            (
                "restart completed",
                text(
                    "- 1 | STARTED | 1.1\n- 2 | DONE | 1.1\n- 3 | STARTED | 1.1",
                    "IN_PROGRESS",
                    "1.1",
                    3,
                ),
                "STARTED completed",
            ),
            (
                "reopen incomplete",
                text("- 1 | REOPENED | 1.1", "IN_PROGRESS", "1.1", 1),
                "REOPENED requires completed inactive",
            ),
            (
                "blocked inactive",
                text("- 1 | BLOCKED | 1.1", "BLOCKED", "1.1", 1),
                "BLOCKED requires active",
            ),
            (
                "replan inactive",
                text("- 1 | NEEDS_REPLAN | 1.1", "NEEDS_REPLAN", "1.1", 1),
                "NEEDS_REPLAN requires active",
            ),
            (
                "after terminal",
                text(
                    "- 1 | STARTED | 1.1\n- 2 | BLOCKED | 1.1\n- 3 | DONE | 1.1",
                    "BLOCKED",
                    "1.1",
                    3,
                ),
                "event follows terminal state",
            ),
        ];
        for (name, candidate, want) in cases {
            let error = ProgressFile::parse(&candidate, &task())
                .unwrap_err()
                .to_string();
            assert!(error.contains(want), "{name}: {error}");
        }
    }

    #[test]
    fn rejects_header_event_disagreement_and_state_machine_in_handoff() {
        let initial = text("- 1 | STARTED | 1.1", "IN_PROGRESS", "1.1", 1);
        let cases = [
            (
                "latest mismatch",
                initial.replacen("latest_event: 1", "latest_event: 2", 1),
                "latest_event does not match",
            ),
            (
                "state mismatch",
                initial.replacen("state: IN_PROGRESS", "state: DONE", 1),
                "disagrees with derived",
            ),
            (
                "current mismatch",
                initial.replacen("current: 1.1", "current: NONE", 1),
                "disagrees with derived",
            ),
            (
                "event in handoff",
                initial.replacen("CURRENT_FAILURE: none", "- 2 | DONE | 1.1", 1),
                "not allowed in handoff",
            ),
            (
                "events heading in handoff",
                initial.replacen("CURRENT_FAILURE: none", "## Events", 1),
                "not allowed in handoff",
            ),
            (
                "fence in handoff",
                initial.replacen("CURRENT_FAILURE: none", "---", 1),
                "not allowed in handoff",
            ),
        ];
        for (name, candidate, want) in cases {
            let error = ProgressFile::parse(&candidate, &task())
                .unwrap_err()
                .to_string();
            assert!(error.contains(want), "{name}: {error}");
        }
    }

    #[test]
    fn reduces_failed_retried_reopened_and_terminal_streams_deterministically() {
        let retry = "- 1 | STARTED | 1.1\n- 2 | FAILED | 1.1\n- 3 | STARTED | 1.1";
        let retry = ProgressFile::parse(&text(retry, "IN_PROGRESS", "1.1", 3), &task()).unwrap();
        assert_eq!(retry.state, State::InProgress);
        assert_eq!(retry.current.as_deref(), Some("1.1"));
        assert!(retry.completed.is_empty());

        let reopened = "- 1 | STARTED | 1.1\n- 2 | DONE | 1.1\n- 3 | REOPENED | 1.1";
        let reopened =
            ProgressFile::parse(&text(reopened, "IN_PROGRESS", "1.1", 3), &task()).unwrap();
        assert_eq!(reopened.state, State::InProgress);
        assert!(reopened.completed.is_empty());

        let blocked = "- 1 | STARTED | 1.1\n- 2 | BLOCKED | 1.1";
        let blocked = ProgressFile::parse(&text(blocked, "BLOCKED", "1.1", 2), &task()).unwrap();
        assert_eq!(blocked.state, State::Blocked);
        assert_eq!(blocked.current.as_deref(), Some("1.1"));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventStatus {
    Started,
    Done,
    Failed,
    Reopened,
    Blocked,
    NeedsReplan,
}
impl EventStatus {
    fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "STARTED" => Ok(Self::Started),
            "DONE" => Ok(Self::Done),
            "FAILED" => Ok(Self::Failed),
            "REOPENED" => Ok(Self::Reopened),
            "BLOCKED" => Ok(Self::Blocked),
            "NEEDS_REPLAN" => Ok(Self::NeedsReplan),
            _ => bail!("progress: unknown event status `{value}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub sequence: u64,
    pub status: EventStatus,
    pub leaf: String,
}

#[derive(Debug, Clone)]
pub struct ProgressFile {
    pub task: String,
    pub state: State,
    pub current: Option<String>,
    pub latest_event: u64,
    pub events: Vec<Event>,
    pub completed: BTreeSet<String>,
    pub handoff: Vec<String>,
}

impl ProgressFile {
    pub fn load(path: &Path, task: &TaskFile) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read progress file {}", path.display()))?;
        Self::parse(&text, task)
    }

    /// Parse every byte of the state machine. Header claims must equal event reduction.
    pub fn parse(text: &str, task: &TaskFile) -> anyhow::Result<Self> {
        let lines: Vec<&str> = text.lines().collect();
        ensure!(
            lines.first() == Some(&"---"),
            "progress: missing opening header fence"
        );
        let close = lines
            .iter()
            .skip(1)
            .position(|line| *line == "---")
            .map(|index| index + 1)
            .ok_or_else(|| anyhow::anyhow!("progress: missing closing header fence"))?;
        let mut headers = BTreeMap::new();
        for line in &lines[1..close] {
            let (key, value) = line
                .split_once(": ")
                .ok_or_else(|| anyhow::anyhow!("progress: malformed header `{line}`"))?;
            ensure!(
                !key.is_empty() && !value.is_empty() && !value.contains('\r'),
                "progress: malformed header `{line}`"
            );
            ensure!(
                matches!(
                    key,
                    "schema" | "task" | "state" | "current" | "latest_event"
                ),
                "progress: unknown header `{key}`"
            );
            ensure!(
                headers.insert(key, value).is_none(),
                "progress: duplicate header `{key}`"
            );
        }
        for key in ["schema", "task", "state", "current", "latest_event"] {
            ensure!(
                headers.contains_key(key),
                "progress: missing header `{key}`"
            );
        }
        ensure!(
            headers["schema"] == SCHEMA,
            "progress: expected schema `{SCHEMA}`"
        );
        ensure!(
            headers["task"] == task.id(),
            "progress: task `{}` does not match `{}`",
            headers["task"],
            task.id()
        );
        let declared_state = State::parse(headers["state"])?;
        let declared_current = match headers["current"] {
            "NONE" => None,
            value => Some(value.to_string()),
        };
        let declared_latest = headers["latest_event"]
            .parse::<u64>()
            .context("progress: latest_event must be a positive integer")?;
        ensure!(
            declared_latest > 0,
            "progress: latest_event must be positive"
        );

        let mut cursor = close + 1;
        ensure!(
            lines.get(cursor) == Some(&""),
            "progress: expected one blank line after header"
        );
        cursor += 1;
        ensure!(
            lines.get(cursor) == Some(&"## Events"),
            "progress: missing or misplaced `## Events`"
        );
        cursor += 1;
        let events_start = cursor;
        while let Some(line) = lines.get(cursor) {
            if line.is_empty() {
                break;
            }
            ensure!(
                line.starts_with("- "),
                "progress: malformed event row `{line}`"
            );
            cursor += 1;
        }
        let events_end = cursor;
        ensure!(
            events_end > events_start,
            "progress: events must not be empty"
        );
        ensure!(
            lines.get(cursor) == Some(&""),
            "progress: expected blank line before handoff"
        );
        cursor += 1;
        ensure!(
            lines.get(cursor) == Some(&"## Handoff"),
            "progress: missing or misplaced `## Handoff`"
        );
        cursor += 1;
        let handoff = lines[cursor..]
            .iter()
            .map(|line| (*line).to_string())
            .collect::<Vec<_>>();
        ensure!(
            !handoff
                .iter()
                .any(|line| line == "## Events" || line == "---" || line.starts_with("- ")),
            "progress: state-machine content is not allowed in handoff"
        );

        let mut events = Vec::new();
        for line in &lines[events_start..events_end] {
            let parts: Vec<_> = line
                .strip_prefix("- ")
                .expect("validated prefix")
                .split(" | ")
                .collect();
            ensure!(
                parts.len() == 3 && parts.iter().all(|part| !part.is_empty()),
                "progress: malformed event row `{line}`"
            );
            let sequence = parts[0]
                .parse::<u64>()
                .with_context(|| format!("progress: invalid event sequence in `{line}`"))?;
            ensure!(sequence > 0, "progress: invalid event sequence in `{line}`");
            events.push(Event {
                sequence,
                status: EventStatus::parse(parts[1])?,
                leaf: parts[2].to_string(),
            });
        }
        let leaves = checklist_leaves(task)?;
        let (state, current, completed) = reduce(&events, &leaves)?;
        ensure!(
            events
                .last()
                .is_some_and(|event| event.sequence == declared_latest),
            "progress: latest_event does not match the final event"
        );
        ensure!(
            state == declared_state,
            "progress: state={} disagrees with derived {}",
            declared_state.as_str(),
            state.as_str()
        );
        ensure!(
            current == declared_current,
            "progress: current={:?} disagrees with derived {:?}",
            declared_current,
            current
        );
        Ok(Self {
            task: task.id().to_string(),
            state,
            current,
            latest_event: declared_latest,
            events,
            completed,
            handoff,
        })
    }
}

fn checklist_leaves(task: &TaskFile) -> anyhow::Result<Vec<String>> {
    let items = taskfile::parse_checklist(&task.checklist);
    ensure!(
        !items.is_empty() && items.iter().all(|item| item.well_formed),
        "progress: task checklist is malformed"
    );
    let flags = taskfile::leaf_flags(&items);
    Ok(items
        .into_iter()
        .zip(flags)
        .filter_map(|(item, leaf)| leaf.then_some(item.id))
        .collect())
}

fn reduce(
    events: &[Event],
    leaves: &[String],
) -> anyhow::Result<(State, Option<String>, BTreeSet<String>)> {
    let known: BTreeSet<_> = leaves.iter().cloned().collect();
    let mut completed = BTreeSet::new();
    let mut active: Option<String> = None;
    let mut terminal = None;
    for (index, event) in events.iter().enumerate() {
        ensure!(
            event.sequence == (index + 1) as u64,
            "progress: event sequence must start at 1 and be contiguous"
        );
        ensure!(
            known.contains(&event.leaf),
            "progress: event references unknown checklist leaf `{}`",
            event.leaf
        );
        ensure!(terminal.is_none(), "progress: event follows terminal state");
        match event.status {
            EventStatus::Started => {
                ensure!(
                    active.is_none(),
                    "progress: STARTED while `{}` is active",
                    active.as_deref().unwrap_or("")
                );
                ensure!(
                    !completed.contains(&event.leaf),
                    "progress: STARTED completed leaf `{}`",
                    event.leaf
                );
                active = Some(event.leaf.clone());
            }
            EventStatus::Done => {
                ensure!(
                    active.as_deref() == Some(&event.leaf),
                    "progress: DONE requires active leaf `{}`",
                    event.leaf
                );
                completed.insert(event.leaf.clone());
                active = None;
            }
            EventStatus::Failed => {
                ensure!(
                    active.as_deref() == Some(&event.leaf),
                    "progress: FAILED requires active leaf `{}`",
                    event.leaf
                );
                active = None;
            }
            EventStatus::Reopened => {
                ensure!(
                    active.is_none() && completed.remove(&event.leaf),
                    "progress: REOPENED requires completed inactive leaf `{}`",
                    event.leaf
                );
                active = Some(event.leaf.clone());
            }
            EventStatus::Blocked => {
                ensure!(
                    active.as_deref() == Some(&event.leaf),
                    "progress: BLOCKED requires active leaf `{}`",
                    event.leaf
                );
                terminal = Some(State::Blocked);
            }
            EventStatus::NeedsReplan => {
                ensure!(
                    active.as_deref() == Some(&event.leaf),
                    "progress: NEEDS_REPLAN requires active leaf `{}`",
                    event.leaf
                );
                terminal = Some(State::NeedsReplan);
            }
        }
    }
    let state = terminal.unwrap_or({
        if completed.len() == leaves.len() {
            State::Done
        } else {
            State::InProgress
        }
    });
    let current = match state {
        State::Done => {
            ensure!(active.is_none(), "progress: DONE has an active leaf");
            None
        }
        _ => active,
    };
    ensure!(
        state != State::InProgress || current.is_some(),
        "progress: IN_PROGRESS requires an active leaf"
    );
    Ok((state, current, completed))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn task() -> TaskFile {
        TaskFile::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/example/README.md"))
            .unwrap()
    }
    fn text(events: &str, state: &str, current: &str, latest: u64) -> String {
        format!(
            "---\nschema: progress/v1\ntask: TASK-042\nstate: {state}\ncurrent: {current}\nlatest_event: {latest}\n---\n\n## Events\n{events}\n\n## Handoff\nCURRENT_FAILURE: none\n"
        )
    }
    #[test]
    fn generated_is_strict_initial_state() {
        let generated =
            generate(&Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/example")).unwrap();
        let parsed = ProgressFile::parse(&generated.body, &task()).unwrap();
        assert_eq!(parsed.current.as_deref(), Some("1.1"));
    }
    #[test]
    fn derives_done_deterministically() {
        let events = "- 1 | STARTED | 1.1\n- 2 | DONE | 1.1\n- 3 | STARTED | 2.1\n- 4 | DONE | 2.1\n- 5 | STARTED | 2.2\n- 6 | DONE | 2.2\n- 7 | STARTED | 2.3\n- 8 | DONE | 2.3\n- 9 | STARTED | 3.1\n- 10 | DONE | 3.1";
        let parsed = ProgressFile::parse(&text(events, "DONE", "NONE", 10), &task()).unwrap();
        assert_eq!(parsed.state, State::Done);
        assert_eq!(parsed.completed.len(), 5);
    }
    #[test]
    fn rejects_header_transition_and_unknown_leaf() {
        assert!(
            ProgressFile::parse(
                &text("- 1 | STARTED | 9.9", "IN_PROGRESS", "9.9", 1),
                &task()
            )
            .is_err()
        );
        assert!(
            ProgressFile::parse(&text("- 1 | DONE | 1.1", "DONE", "NONE", 1), &task()).is_err()
        );
        assert!(
            ProgressFile::parse(&text("- 1 | STARTED | 1.1", "DONE", "NONE", 1), &task()).is_err()
        );
    }
}
