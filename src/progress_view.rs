//! The task checklist joined with `progress/v1`: which leaves are done, which one the agent is on
//! right now, which failed and await a retry. A *view* — it changes nothing and proves nothing
//! (the gate does that); it exists so an operator can read the run's position at a glance.

use anyhow::ensure;
use serde::Serialize;
use std::collections::BTreeSet;

use crate::progress::{EventStatus, ProgressFile, State};
use crate::taskfile::{self, TaskFile};

/// Where one checklist item stands, derived from the event stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    /// Not started (or a parent none of whose leaves has moved).
    Pending,
    /// The active leaf, or a parent with work under it.
    InProgress,
    /// The leaf's latest event is `FAILED` and it has not been restarted.
    Failed,
    /// The active leaf when the run is `BLOCKED` (terminal), or a parent above it.
    Blocked,
    /// The active leaf when the run is `NEEDS_REPLAN` (terminal), or a parent above it.
    NeedsReplan,
    /// `DONE` (leaf), or every leaf beneath is done (parent).
    Done,
}

impl ItemStatus {
    /// The checkbox as rendered: `[x]` done, `[>]` in progress, `[!]` failed/blocked,
    /// `[?]` needs replan, `[ ]` pending.
    pub fn marker(self) -> &'static str {
        match self {
            Self::Pending => "[ ]",
            Self::InProgress => "[>]",
            Self::Failed | Self::Blocked => "[!]",
            Self::NeedsReplan => "[?]",
            Self::Done => "[x]",
        }
    }

    /// The word appended to a leaf that is not simply pending or done.
    fn annotation(self) -> Option<&'static str> {
        match self {
            Self::InProgress => Some("in progress"),
            Self::Failed => Some("failed, not retried"),
            Self::Blocked => Some("blocked"),
            Self::NeedsReplan => Some("needs replan"),
            Self::Pending | Self::Done => None,
        }
    }
}

/// One checklist line with its derived status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemView {
    pub id: String,
    pub text: String,
    pub depth: usize,
    pub leaf: bool,
    pub status: ItemStatus,
    /// The README checklist row with its checkbox replaced by [`ItemStatus::marker`]:
    /// `    - [>] **1.2** Configure …`. Same indent and shape as the task file, so it reads
    /// (and diffs) against the README the agent is working from.
    pub line: String,
}

/// The whole checklist plus the progress header it was joined with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChecklistView {
    pub task: String,
    /// `progress/v1` state, or `None` when no valid progress file was available.
    pub state: Option<String>,
    /// The active leaf, when there is one.
    pub current: Option<String>,
    pub latest_event: Option<u64>,
    /// Leaves done / leaves total.
    pub done: usize,
    pub total: usize,
    pub items: Vec<ItemView>,
}

impl ChecklistView {
    /// Join the task checklist with a parsed progress file.
    pub fn build(task: &TaskFile, progress: Option<&ProgressFile>) -> anyhow::Result<Self> {
        let parsed = taskfile::parse_checklist(&task.checklist);
        ensure!(
            !parsed.is_empty() && parsed.iter().all(|item| item.well_formed),
            "progress view: task checklist is malformed"
        );
        let leaf_flags = taskfile::leaf_flags(&parsed);

        let completed: BTreeSet<&str> = progress
            .map(|p| p.completed.iter().map(String::as_str).collect())
            .unwrap_or_default();
        let current = progress.and_then(|p| p.current.as_deref());
        let active_status = match progress.map(|p| p.state) {
            Some(State::Blocked) => ItemStatus::Blocked,
            Some(State::NeedsReplan) => ItemStatus::NeedsReplan,
            _ => ItemStatus::InProgress,
        };
        // A leaf whose newest event is FAILED, and which is not active or completed, sits in the
        // gap between an attempt and its retry: worth naming, since "pending" would hide it.
        let failed: BTreeSet<&str> = progress
            .map(|p| {
                let mut latest: std::collections::BTreeMap<&str, EventStatus> = Default::default();
                for event in &p.events {
                    latest.insert(event.leaf.as_str(), event.status);
                }
                latest
                    .into_iter()
                    .filter(|(_, status)| *status == EventStatus::Failed)
                    .map(|(leaf, _)| leaf)
                    .collect()
            })
            .unwrap_or_default();

        let mut items: Vec<ItemView> = parsed
            .iter()
            .zip(&leaf_flags)
            .map(|(item, &leaf)| {
                let id = item.id.as_str();
                let status = if !leaf {
                    ItemStatus::Pending // parents are filled in below
                } else if completed.contains(id) {
                    ItemStatus::Done
                } else if current == Some(id) {
                    active_status
                } else if failed.contains(id) {
                    ItemStatus::Failed
                } else {
                    ItemStatus::Pending
                };
                ItemView {
                    id: item.id.clone(),
                    text: item.text.clone(),
                    depth: item.depth,
                    leaf,
                    status,
                    line: String::new(), // filled once every status is final
                }
            })
            .collect();

        // Parents, deepest first so a parent reads its children's final status.
        for index in (0..items.len()).rev() {
            if items[index].leaf {
                continue;
            }
            let depth = items[index].depth;
            let children: Vec<ItemStatus> = items[index + 1..]
                .iter()
                .take_while(|item| item.depth > depth)
                .filter(|item| item.leaf)
                .map(|item| item.status)
                .collect();
            items[index].status = parent_status(&children);
        }
        for (item, parsed) in items.iter_mut().zip(&parsed) {
            item.line = format!(
                "{}- {} **{}** {}",
                " ".repeat(parsed.indent),
                item.status.marker(),
                item.id,
                item.text
            );
        }

        let total = items.iter().filter(|item| item.leaf).count();
        ensure!(total > 0, "progress view: task checklist has no leaf items");
        let done = items
            .iter()
            .filter(|item| item.leaf && item.status == ItemStatus::Done)
            .count();
        Ok(Self {
            task: task.id().to_string(),
            state: progress.map(|p| p.state.as_str().to_string()),
            current: current.map(str::to_string),
            latest_event: progress.map(|p| p.latest_event),
            done,
            total,
            items,
        })
    }

    /// The one-line summary: state, current leaf, done count, latest event.
    pub fn summary(&self) -> String {
        let mut out = format!(
            "progress: {}  done {}/{}",
            self.state.as_deref().unwrap_or("UNKNOWN"),
            self.done,
            self.total
        );
        if let Some(current) = &self.current {
            out.push_str(&format!("  current {current}"));
        }
        if let Some(latest) = self.latest_event {
            out.push_str(&format!("  latest_event {latest}"));
        }
        out
    }

    /// The checklist as it stands in the README, one row per item with the checkbox replaced by
    /// the status marker. Leaves that are neither pending nor done carry a trailing
    /// `<- in progress` / `<- failed, not retried` / `<- blocked` / `<- needs replan` so the eye
    /// lands on them.
    pub fn render(&self) -> Vec<String> {
        let mut lines = vec![self.summary()];
        for item in &self.items {
            let mut line = item.line.clone();
            if item.leaf
                && let Some(note) = item.status.annotation()
            {
                line.push_str("  <- ");
                line.push_str(note);
            }
            lines.push(line);
        }
        lines
    }
}

/// A parent's status is its leaves' collective status.
fn parent_status(children: &[ItemStatus]) -> ItemStatus {
    if children.is_empty() {
        return ItemStatus::Pending;
    }
    if children.iter().all(|status| *status == ItemStatus::Done) {
        return ItemStatus::Done;
    }
    if children.contains(&ItemStatus::Blocked) {
        return ItemStatus::Blocked;
    }
    if children.contains(&ItemStatus::NeedsReplan) {
        return ItemStatus::NeedsReplan;
    }
    if children.iter().any(|status| *status != ItemStatus::Pending) {
        return ItemStatus::InProgress;
    }
    ItemStatus::Pending
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn task() -> TaskFile {
        TaskFile::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/example/README.md"))
            .unwrap()
    }

    fn progress(events: &str, state: &str, current: &str, latest: u64) -> ProgressFile {
        let text = format!(
            "---\nschema: progress/v1\ntask: TASK-042\nstate: {state}\ncurrent: {current}\nlatest_event: {latest}\n---\n\n## Events\n{events}\n\n## Handoff\nCURRENT_FAILURE: none\n"
        );
        ProgressFile::parse(&text, &task()).unwrap()
    }

    fn status_of(view: &ChecklistView, id: &str) -> ItemStatus {
        view.items.iter().find(|item| item.id == id).unwrap().status
    }

    #[test]
    fn the_initial_stream_marks_the_first_leaf_in_progress_and_its_parent_too() {
        let view = ChecklistView::build(
            &task(),
            Some(&progress("- 1 | STARTED | 1.1", "IN_PROGRESS", "1.1", 1)),
        )
        .unwrap();
        assert_eq!(view.state.as_deref(), Some("IN_PROGRESS"));
        assert_eq!(view.current.as_deref(), Some("1.1"));
        assert_eq!((view.done, view.total), (0, 5));
        assert_eq!(status_of(&view, "1.1"), ItemStatus::InProgress);
        assert_eq!(status_of(&view, "1"), ItemStatus::InProgress);
        assert_eq!(status_of(&view, "2"), ItemStatus::Pending);
        assert_eq!(status_of(&view, "2.1"), ItemStatus::Pending);
        let rendered = view.render();
        assert!(rendered[0].starts_with("progress: IN_PROGRESS  done 0/5  current 1.1"));
        let leaf = rendered
            .iter()
            .find(|l| l.contains("[>] **1.1** "))
            .unwrap();
        assert!(leaf.starts_with("    - [>] **1.1** "), "{leaf}");
        assert!(leaf.ends_with("<- in progress"), "{leaf}");
        let parent = rendered.iter().find(|l| l.contains("[>] **1** ")).unwrap();
        assert!(parent.starts_with("- [>] **1** "), "{parent}");
        assert!(
            !parent.contains("<-"),
            "parents carry no annotation: {parent}"
        );
    }

    #[test]
    fn done_leaves_roll_up_into_their_parent() {
        let events = "- 1 | STARTED | 1.1\n- 2 | DONE | 1.1\n- 3 | STARTED | 2.1\n- 4 | DONE | 2.1\n- 5 | STARTED | 2.2";
        let view = ChecklistView::build(&task(), Some(&progress(events, "IN_PROGRESS", "2.2", 5)))
            .unwrap();
        assert_eq!(view.done, 2);
        assert_eq!(status_of(&view, "1"), ItemStatus::Done);
        assert_eq!(status_of(&view, "1.1"), ItemStatus::Done);
        assert_eq!(status_of(&view, "2"), ItemStatus::InProgress);
        assert_eq!(status_of(&view, "2.1"), ItemStatus::Done);
        assert_eq!(status_of(&view, "2.2"), ItemStatus::InProgress);
        assert_eq!(status_of(&view, "2.3"), ItemStatus::Pending);
        assert_eq!(status_of(&view, "3"), ItemStatus::Pending);
    }

    #[test]
    fn a_failed_leaf_awaiting_retry_is_named_not_hidden() {
        let events = "- 1 | STARTED | 1.1\n- 2 | FAILED | 1.1\n- 3 | STARTED | 2.1";
        let view = ChecklistView::build(&task(), Some(&progress(events, "IN_PROGRESS", "2.1", 3)))
            .unwrap();
        assert_eq!(status_of(&view, "1.1"), ItemStatus::Failed);
        assert_eq!(status_of(&view, "1"), ItemStatus::InProgress);
        let line = view
            .render()
            .into_iter()
            .find(|l| l.contains("[!] **1.1** "))
            .unwrap();
        assert!(line.ends_with("<- failed, not retried"), "{line}");
        // once restarted it is simply in progress again
        let retried = "- 1 | STARTED | 1.1\n- 2 | FAILED | 1.1\n- 3 | STARTED | 1.1";
        let view = ChecklistView::build(&task(), Some(&progress(retried, "IN_PROGRESS", "1.1", 3)))
            .unwrap();
        assert_eq!(status_of(&view, "1.1"), ItemStatus::InProgress);
    }

    #[test]
    fn terminal_states_colour_the_active_leaf_and_its_parent() {
        let blocked = "- 1 | STARTED | 1.1\n- 2 | BLOCKED | 1.1";
        let view =
            ChecklistView::build(&task(), Some(&progress(blocked, "BLOCKED", "1.1", 2))).unwrap();
        assert_eq!(status_of(&view, "1.1"), ItemStatus::Blocked);
        assert_eq!(status_of(&view, "1"), ItemStatus::Blocked);
        assert!(view.render()[0].starts_with("progress: BLOCKED"));
        let replan = "- 1 | STARTED | 1.1\n- 2 | NEEDS_REPLAN | 1.1";
        let view = ChecklistView::build(&task(), Some(&progress(replan, "NEEDS_REPLAN", "1.1", 2)))
            .unwrap();
        assert_eq!(status_of(&view, "1.1"), ItemStatus::NeedsReplan);
        assert_eq!(status_of(&view, "1"), ItemStatus::NeedsReplan);
    }

    #[test]
    fn a_finished_run_is_all_done() {
        let events = "- 1 | STARTED | 1.1\n- 2 | DONE | 1.1\n- 3 | STARTED | 2.1\n- 4 | DONE | 2.1\n- 5 | STARTED | 2.2\n- 6 | DONE | 2.2\n- 7 | STARTED | 2.3\n- 8 | DONE | 2.3\n- 9 | STARTED | 3.1\n- 10 | DONE | 3.1";
        let view =
            ChecklistView::build(&task(), Some(&progress(events, "DONE", "NONE", 10))).unwrap();
        assert_eq!((view.done, view.total), (5, 5));
        assert!(view.current.is_none());
        assert!(
            view.items
                .iter()
                .all(|item| item.status == ItemStatus::Done)
        );
        assert_eq!(
            view.render()[0],
            "progress: DONE  done 5/5  latest_event 10"
        );
    }

    #[test]
    fn the_json_shape_names_every_item_status() {
        let view = ChecklistView::build(
            &task(),
            Some(&progress("- 1 | STARTED | 1.1", "IN_PROGRESS", "1.1", 1)),
        )
        .unwrap();
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["state"], "IN_PROGRESS");
        assert_eq!(json["done"], 0);
        assert_eq!(json["total"], 5);
        assert_eq!(json["items"][0]["id"], "1");
        assert_eq!(json["items"][0]["leaf"], false);
        assert_eq!(json["items"][0]["status"], "in_progress");
        assert_eq!(json["items"][1]["id"], "1.1");
        assert_eq!(json["items"][1]["status"], "in_progress");
        assert_eq!(json["items"][2]["status"], "pending");
        assert_eq!(json["items"][0]["line"], "- [>] **1** Reproduce.");
        assert!(
            json["items"][1]["line"]
                .as_str()
                .unwrap()
                .starts_with("    - [>] **1.1** ")
        );
    }

    /// Every row keeps the README's shape and indent; only the checkbox byte moves. A `DONE`
    /// stream therefore renders as the README with every box ticked.
    fn readme_rows() -> Vec<String> {
        taskfile::parse_checklist(&task().checklist)
            .into_iter()
            .map(|item| item.raw)
            .collect()
    }

    #[test]
    fn lines_are_the_readme_rows_with_only_the_checkbox_changed() {
        let events = "- 1 | STARTED | 1.1\n- 2 | DONE | 1.1\n- 3 | STARTED | 2.1\n- 4 | DONE | 2.1\n- 5 | STARTED | 2.2\n- 6 | DONE | 2.2\n- 7 | STARTED | 2.3\n- 8 | DONE | 2.3\n- 9 | STARTED | 3.1\n- 10 | DONE | 3.1";
        let view =
            ChecklistView::build(&task(), Some(&progress(events, "DONE", "NONE", 10))).unwrap();
        let lines: Vec<&str> = view.items.iter().map(|item| item.line.as_str()).collect();
        let ticked: Vec<String> = readme_rows()
            .iter()
            .map(|raw| raw.replacen("- [ ]", "- [x]", 1))
            .collect();
        assert_eq!(lines, ticked);
        // and with nothing done, the rows are the README verbatim
        let view = ChecklistView::build(&task(), None).unwrap();
        let lines: Vec<&str> = view.items.iter().map(|item| item.line.as_str()).collect();
        assert_eq!(lines, readme_rows());
    }

    #[test]
    fn parent_status_table() {
        use ItemStatus::*;
        assert_eq!(parent_status(&[]), Pending);
        assert_eq!(parent_status(&[Pending, Pending]), Pending);
        assert_eq!(parent_status(&[Done, Done]), Done);
        assert_eq!(parent_status(&[Done, Pending]), InProgress);
        assert_eq!(parent_status(&[Failed, Pending]), InProgress);
        assert_eq!(parent_status(&[InProgress, Pending]), InProgress);
        assert_eq!(parent_status(&[Done, Blocked]), Blocked);
        assert_eq!(parent_status(&[Done, NeedsReplan]), NeedsReplan);
    }
}
