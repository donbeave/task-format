//! Read-only Git inspection for the task scope check.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{capture, ignore::BaseIgnores};

pub(crate) fn in_dir(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command.current_dir(dir).args(args);
    command
}

pub(crate) fn output(command: &mut Command) -> anyhow::Result<String> {
    let captured = capture(command)?;
    if captured.status != 0 {
        anyhow::bail!(
            "git failed (exit={}): {}",
            captured.status,
            captured.stderr.trim_end()
        );
    }
    Ok(captured.stdout)
}

pub(crate) fn resolves_commit(dir: &Path, base: &str) -> bool {
    resolve_commit(dir, base).is_ok()
}

pub(crate) fn resolve_commit(dir: &Path, base: &str) -> anyhow::Result<String> {
    let revision = format!("{base}^{{commit}}");
    let resolved = output(&mut in_dir(
        dir,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &revision,
        ],
    ))?;
    Ok(resolved.trim().to_string())
}

/// Every changed, staged, and untracked path relative to `base`, sorted and deduplicated.
/// Untracked files use only per-directory ignore rules; an executor-controlled ignore file,
/// `.git/info/exclude`, or global exclude file cannot hide a path from scope validation.
pub(crate) fn changed_files(dir: &Path, base: &str) -> anyhow::Result<Vec<String>> {
    let enumerations: [(&[&str], bool); 4] = [
        (
            &["diff", "--no-renames", "--name-only", "-z", base, "--"],
            false,
        ),
        (
            &[
                "diff",
                "--no-renames",
                "--name-only",
                "-z",
                "--cached",
                "--",
            ],
            false,
        ),
        (
            &[
                "ls-files",
                "--others",
                "--exclude-per-directory=.gitignore",
                "-z",
            ],
            false,
        ),
        (
            &[
                "ls-files",
                "--others",
                "-z",
                "--",
                ":(top,glob)**/.gitignore",
            ],
            true,
        ),
    ];
    let mut files = Vec::new();
    for (args, unfiltered_ignore_scan) in enumerations {
        let output = output(&mut in_dir(dir, args))?;
        let listed = output
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        if unfiltered_ignore_scan {
            files.extend(drop_dirs_ignored_at_base(dir, base, listed));
        } else {
            files.extend(listed);
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn drop_dirs_ignored_at_base(dir: &Path, base: &str, candidates: Vec<String>) -> Vec<String> {
    if candidates.is_empty() {
        return candidates;
    }
    let Ok(rules) = BaseIgnores::load(dir, base) else {
        return candidates;
    };
    let mut decided: HashMap<PathBuf, bool> = HashMap::new();
    candidates
        .into_iter()
        .filter(|path| {
            let Some(parent) = Path::new(path)
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            else {
                return true;
            };
            !*decided
                .entry(parent.to_path_buf())
                .or_insert_with(|| rules.dir_ignored_at_base(parent).unwrap_or(false))
        })
        .collect()
}

pub(crate) fn hidden_index_entries(dir: &Path) -> anyhow::Result<Vec<String>> {
    Ok(output(&mut in_dir(dir, &["ls-files", "-v"]))?
        .lines()
        .filter(|line| {
            let mut chars = line.chars();
            matches!(chars.next(), Some(tag) if tag == 'S' || tag.is_ascii_lowercase())
                && chars.next() == Some(' ')
        })
        .map(str::to_string)
        .collect())
}
