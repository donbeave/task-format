//! `taskfmt init` — create `/progress/progress.md` once from the task README.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;

use taskfmt::progress;
use taskfmt::redact;

use super::verify::{resolve_progress, resolve_task_dir};

/// Exit code when the progress file already exists (init runs at most once per path).
pub const EXIT_ALREADY_INITIALIZED: i32 = 64;

/// Create the initial progress file if `out` does not exist yet.
pub fn create_once(task_dir: &Path, out: &Path) -> anyhow::Result<i32> {
    if out.is_file() {
        redact::eemit(&format!(
            "taskfmt init: {} already exists; init runs only once",
            out.display()
        ));
        return Ok(EXIT_ALREADY_INITIALIZED);
    }
    let generated = progress::generate(task_dir)?;
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out)
        .with_context(|| format!("creating {}", out.display()))?;
    file.write_all(generated.body.as_bytes())
        .with_context(|| format!("writing {}", out.display()))?;
    redact::eemit(&format!(
        "PROGRESS {} task={} current={}",
        out.display(),
        generated.task,
        generated.first_leaf
    ));
    Ok(0)
}

/// In-container CLI entry.
pub fn run(task_dir: Option<&Path>, out: Option<&Path>) -> anyhow::Result<i32> {
    let task_dir = resolve_task_dir(task_dir)?;
    let out = out
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(resolve_progress(None)));
    create_once(&task_dir, &out)
}

/// Shared by tests, selftest, and legacy call sites (always uses create-once semantics).
pub fn generate_and_write(task_dir: &Path, out: Option<&Path>) -> anyhow::Result<i32> {
    match out {
        Some(path) => create_once(task_dir, path),
        None => {
            let generated = progress::generate(task_dir)?;
            print!("{}", generated.body);
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_init_refuses_with_64() {
        let dir = tempfile::tempdir().unwrap();
        let task = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../harness/testdata/example");
        let out = dir.path().join("progress.md");
        assert_eq!(create_once(&task, &out).unwrap(), 0);
        assert_eq!(create_once(&task, &out).unwrap(), EXIT_ALREADY_INITIALIZED);
    }
}
