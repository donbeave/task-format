//! `taskfmt lint [paths..]` — lint task packages (in-container).

use std::path::{Path, PathBuf};

use taskfmt::{lint, redact};

/// Lint explicit package paths (in-container `taskfmt lint`). No experiment manifest required.
pub fn run_paths(json: bool, paths: &[PathBuf]) -> anyhow::Result<i32> {
    let targets: Vec<PathBuf> = if paths.is_empty() {
        vec![default_container_task_dir()]
    } else {
        paths.to_vec()
    };
    let mut failed = 0usize;
    for target in &targets {
        let report = lint::lint_path(target);
        if json {
            redact::emit(&report.render_json());
        } else {
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

fn default_container_task_dir() -> PathBuf {
    if let Ok(value) = std::env::var("TASKFMT_TASK_DIR")
        && !value.is_empty()
    {
        return PathBuf::from(value);
    }
    Path::new("/task").to_path_buf()
}
