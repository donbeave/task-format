//! `taskfmt status` — parse and validate coordination progress (not host run completion).

use std::path::{Path, PathBuf};

use anyhow::Context;

use taskfmt::progress::ProgressFile;
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
    let report = StatusReport {
        task: parsed.task.clone(),
        state: parsed.state.as_str().to_string(),
        current: parsed.current.clone(),
        latest_event: parsed.latest_event,
        completed: parsed.completed.into_iter().collect(),
    };
    if json {
        redact::emit(&serde_json::to_string(&report)?);
    } else {
        let current = report.current.as_deref().unwrap_or("NONE");
        redact::emit(&format!(
            "task={} state={} current={} latest_event={}",
            report.task, report.state, current, report.latest_event
        ));
    }
    Ok(0)
}
