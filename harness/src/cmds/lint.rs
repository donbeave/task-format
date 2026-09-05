//! `taskfmt lint [TASKS..]` — lint task packages under the configured tasks dir.

use crate::cmds::{Ctx, all_task_dirs, resolve_task_arg};
use crate::lint;
use crate::redact;
use crate::selection;

/// Lint each named task (default: all). Exit 1 when any package has an ERROR.
pub fn run(ctx: &Ctx, json: bool, tasks: &[String]) -> anyhow::Result<i32> {
    let resolved = ctx.load()?;
    let tasks_dir = resolved.tasks_dir();

    let targets: Vec<std::path::PathBuf> = if tasks.is_empty() {
        all_task_dirs(&tasks_dir)?
    } else {
        // a path (a task dir or its README.md) is accepted verbatim; everything else is a
        // selection token (`all`, `1-3`, `TASK-101`, …)
        let mut as_paths: Vec<std::path::PathBuf> = Vec::new();
        let mut tokens: Vec<String> = Vec::new();
        for task in tasks {
            let candidate = std::path::Path::new(task);
            if candidate.is_dir() || candidate.is_file() {
                as_paths.push(resolve_task_arg(&tasks_dir, task)?);
            } else {
                tokens.push(task.clone());
            }
        }
        let ids = selection::resolve(&tokens, &tasks_dir)?;
        let mut out = as_paths;
        for id in ids {
            out.push(resolve_task_arg(&tasks_dir, &id)?);
        }
        out
    };
    if targets.is_empty() {
        anyhow::bail!("no task packages found under {}", tasks_dir.display());
    }

    let mut failed = 0usize;
    for target in &targets {
        let report = lint::lint_path(target);
        if json {
            redact::emit(&report.render_json());
        } else {
            // A report may be empty on success.  Always label it so batched output remains
            // attributable and shell users never confuse adjacent package summaries.
            redact::emit(&format!("PACKAGE {}", report.target.display()));
            redact::emit_lines(report.render().lines());
        }
        if !report.passed() {
            failed += 1;
        }
    }
    if failed > 0 && !json {
        redact::emit(&format!(
            "{failed} of {} task package(s) failed lint",
            targets.len()
        ));
        return Ok(1);
    }
    Ok(0)
}
