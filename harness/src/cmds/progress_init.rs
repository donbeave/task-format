//! `taskfmt progress-init <TASK> [-o FILE]` — README.md -> the initial progress.md.

use std::path::Path;

use crate::cmds::{Ctx, resolve_task_arg};
use crate::progress;
use crate::redact;

pub fn run(ctx: &Ctx, task: &str, out: Option<&Path>) -> anyhow::Result<i32> {
    let resolved = ctx.load()?;
    let task_dir = resolve_task_arg(&resolved.tasks_dir(), task)?;
    generate_and_write(&task_dir, out)
}

/// Shared by `progress-init`, `run` and `selftest`.
pub fn generate_and_write(task_dir: &Path, out: Option<&Path>) -> anyhow::Result<i32> {
    let generated = progress::generate(task_dir)?;
    match out {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            crate::ops::write_file(path, &generated.body)?;
            redact::eemit(&format!(
                "PROGRESS {} task={} current={}",
                path.display(),
                generated.task,
                generated.first_leaf
            ));
        }
        None => print!("{}", generated.body),
    }
    Ok(0)
}
