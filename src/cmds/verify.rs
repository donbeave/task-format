use std::path::{Path, PathBuf};

use anyhow::{Context, ensure};

use crate::cli::VerifyOptions;
use crate::gate::{self, GateOpts};

pub fn run(options: VerifyOptions) -> anyhow::Result<i32> {
    let root = existing_dir(&options.root, "workspace root")?;
    let task_dir = existing_dir(&options.task_dir, "task directory")?;
    let progress = match options.progress {
        Some(path) => Some(existing_file(&path, "progress file")?),
        None => None,
    };
    ensure!(!options.base.trim().is_empty(), "--base must not be empty");

    let output = gate::run(GateOpts {
        root,
        task_dir,
        progress,
        base: options.base,
    });
    print!("{}", output.text);
    Ok(output.exit)
}

fn existing_dir(path: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let resolved = path
        .canonicalize()
        .with_context(|| format!("cannot resolve {name} {}", path.display()))?;
    ensure!(
        resolved.is_dir(),
        "{name} is not a directory: {}",
        path.display()
    );
    Ok(resolved)
}

fn existing_file(path: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let resolved = path
        .canonicalize()
        .with_context(|| format!("cannot resolve {name} {}", path.display()))?;
    ensure!(
        resolved.is_file(),
        "{name} is not a file: {}",
        path.display()
    );
    Ok(resolved)
}
