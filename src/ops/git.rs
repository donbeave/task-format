//! Read-only Git inspection for the task scope check.

use std::path::Path;
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

pub(crate) fn output_bytes(command: &mut Command) -> anyhow::Result<Vec<u8>> {
    let captured = command.output()?;
    if !captured.status.success() {
        anyhow::bail!(
            "git failed (exit={}): {}",
            captured.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&captured.stderr).trim_end()
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
/// Untracked files are enumerated without worktree ignore rules, then filtered only by ignore rules
/// stored in `base`; executor-controlled `.gitignore` files cannot hide paths from scope validation.
pub(crate) fn changed_files(dir: &Path, base: &str) -> anyhow::Result<Vec<String>> {
    let comparisons: [&[&str]; 2] = [
        &["diff", "--no-renames", "--name-only", "-z", base, "--"],
        &[
            "diff",
            "--no-renames",
            "--name-only",
            "-z",
            "--cached",
            "--",
        ],
    ];
    let mut files = Vec::new();
    for (args, label) in comparisons.into_iter().zip([
        "git diff --name-only -z from the supplied base",
        "git diff --cached --name-only -z",
    ]) {
        let output = output_bytes(&mut in_dir(dir, args))?;
        files.extend(nul_paths(&output, label)?);
    }
    let untracked = nul_paths(
        &output_bytes(&mut in_dir(dir, &["ls-files", "--others", "-z"]))?,
        "git ls-files --others -z",
    )?;
    files.extend(drop_paths_ignored_at_base(dir, base, untracked));
    files.sort();
    files.dedup();
    Ok(files)
}

fn nul_paths(output: &[u8], source: &str) -> anyhow::Result<Vec<String>> {
    output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            std::str::from_utf8(path)
                .map(str::to_string)
                .map_err(|error| {
                    anyhow::anyhow!(
                        "{source} returned a path that is not valid UTF-8 at byte {}; task scope validation requires UTF-8 paths",
                        error.valid_up_to()
                    )
                })
        })
        .collect()
}

fn drop_paths_ignored_at_base(dir: &Path, base: &str, candidates: Vec<String>) -> Vec<String> {
    if candidates.is_empty() {
        return candidates;
    }
    let Ok(rules) = BaseIgnores::load(dir, base) else {
        return candidates;
    };
    let Ok(ignored) = rules.paths_ignored_at_base(&candidates) else {
        return candidates;
    };
    if ignored.len() != candidates.len() {
        return candidates;
    }
    candidates
        .into_iter()
        .zip(ignored)
        .filter_map(|(path, ignored)| (!ignored).then_some(path))
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

#[cfg(test)]
mod tests {
    use super::nul_paths;

    #[test]
    fn nul_paths_rejects_invalid_utf8_instead_of_replacing_bytes() {
        let error = nul_paths(b"valid\0\xff\0", "git ls-files --others -z")
            .unwrap_err()
            .to_string();

        assert!(error.contains("git ls-files --others -z"), "{error}");
        assert!(error.contains("not valid UTF-8"), "{error}");
    }
}
