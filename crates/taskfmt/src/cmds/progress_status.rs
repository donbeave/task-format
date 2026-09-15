//! `taskfmt status` — parse and validate coordination progress (not host run completion).
//!
//! Text mode prints the derived header line and then the task checklist with each item's
//! position — `[x]` done, `[>]` in progress, `[!]` failed or blocked, `[ ]` pending — so the
//! agent (or an operator shelled into the container) sees what is finished and what is being
//! worked on right now. JSON mode carries the same checklist under `checklist`.

use std::path::{Path, PathBuf};

use anyhow::Context;

use taskfmt::progress::ProgressFile;
use taskfmt::progress_view::{ChecklistView, ItemView};
use taskfmt::redact;
use taskfmt::taskfile::TaskFile;

use super::verify::{resolve_progress, resolve_task_dir};

#[derive(serde::Serialize)]
struct StatusReport {
    task: String,
    state: String,
    current: Option<String>,
    latest_event: u64,
    completed: Vec<String>,
    /// Leaves done / total.
    done: usize,
    total: usize,
    /// Every checklist item with its derived status.
    checklist: Vec<ItemView>,
}

pub fn run(json: bool, task_dir: Option<&Path>, progress: Option<&Path>) -> anyhow::Result<i32> {
    let task_dir = resolve_task_dir(task_dir)?;
    let progress_path = progress
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(resolve_progress(None)));
    let readme = task_dir.join("README.md");
    let task = TaskFile::load(&readme)
        .with_context(|| format!("cannot load task from {}", readme.display()))?;
    let parsed = ProgressFile::load(&progress_path, &task).with_context(|| {
        format!(
            "taskfmt status: invalid or missing progress at {}",
            progress_path.display()
        )
    })?;
    let view = ChecklistView::build(&task, Some(&parsed))?;
    let report = StatusReport {
        task: parsed.task.clone(),
        state: parsed.state.as_str().to_string(),
        current: parsed.current.clone(),
        latest_event: parsed.latest_event,
        completed: parsed.completed.into_iter().collect(),
        done: view.done,
        total: view.total,
        checklist: view.items.clone(),
    };
    if json {
        redact::emit(&serde_json::to_string(&report)?);
    } else {
        let current = report.current.as_deref().unwrap_or("NONE");
        redact::emit(&format!(
            "task={} state={} current={} latest_event={} done={}/{}",
            report.task, report.state, current, report.latest_event, report.done, report.total
        ));
        // `render()` leads with its own summary line; the key=value line above is the stable
        // first line callers already parse, so print the checklist body only.
        redact::emit_lines(view.render().into_iter().skip(1));
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reports_the_checklist_position() {
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../harness/testdata/example");
        let dir = tempfile::tempdir().unwrap();
        let progress = dir.path().join("progress.md");
        std::fs::write(
            &progress,
            "---\nschema: progress/v1\ntask: TASK-042\nstate: IN_PROGRESS\ncurrent: 2.1\nlatest_event: 3\n---\n\n## Events\n- 1 | STARTED | 1.1\n- 2 | DONE | 1.1\n- 3 | STARTED | 2.1\n\n## Handoff\nCURRENT_FAILURE: none\n",
        )
        .unwrap();
        assert_eq!(run(false, Some(&example), Some(&progress)).unwrap(), 0);
        assert_eq!(run(true, Some(&example), Some(&progress)).unwrap(), 0);
        // an invalid progress file is still an error: the agent must fix its state machine
        std::fs::write(&progress, "nope").unwrap();
        assert!(run(false, Some(&example), Some(&progress)).is_err());
    }
}
