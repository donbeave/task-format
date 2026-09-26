use std::path::Path;

use anyhow::{Context, bail, ensure};
use serde::Serialize;

use crate::lint;
use crate::progress::ProgressFile;
use crate::progress_view::{ChecklistView, ItemView};
use crate::taskfile::TaskFile;

#[derive(Serialize)]
struct StatusReport {
    task: String,
    state: String,
    current: Option<String>,
    latest_event: u64,
    completed: Vec<String>,
    completed_leaves: usize,
    total_leaves: usize,
    /// Whole percentage, rounded to nearest integer with half values rounded up.
    percent: usize,
    checklist: Vec<ItemView>,
}

pub fn run(task_dir: &Path, progress_path: &Path, json: bool) -> anyhow::Result<i32> {
    ensure!(
        task_dir.is_dir(),
        "task directory does not exist: {}",
        task_dir.display()
    );
    ensure!(
        progress_path.is_file(),
        "progress file does not exist: {}",
        progress_path.display()
    );

    let lint = lint::lint_path(task_dir);
    if !lint.passed() {
        bail!("task package failed lint:\n{}", lint.render().trim_end());
    }
    let readme = task_dir.join("README.md");
    let task = TaskFile::load(&readme)
        .with_context(|| format!("cannot load task from {}", readme.display()))?;
    let progress = ProgressFile::load(progress_path, &task).with_context(|| {
        format!(
            "invalid or missing progress file {}",
            progress_path.display()
        )
    })?;
    let view = ChecklistView::build(&task, Some(&progress))?;
    ensure!(view.total > 0, "task checklist has no leaf items");

    let percent = (view.done * 100 + view.total / 2) / view.total;
    let report = StatusReport {
        task: progress.task,
        state: progress.state.as_str().to_string(),
        current: progress.current,
        latest_event: progress.latest_event,
        completed: progress.completed.into_iter().collect(),
        completed_leaves: view.done,
        total_leaves: view.total,
        percent,
        checklist: view.items.clone(),
    };
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!(
            "task={} state={} current={} completed={}/{} percent={} latest_event={}",
            report.task,
            report.state,
            report.current.as_deref().unwrap_or("NONE"),
            report.completed_leaves,
            report.total_leaves,
            report.percent,
            report.latest_event
        );
        for line in view.render().into_iter().skip(1) {
            println!("{line}");
        }
    }
    Ok(0)
}
