//! Task selection: `"all"`, ranges (`1-3`, `TASK-002..TASK-004`) and single ids, ordered by id and
//! deduplicated.

use std::path::Path;

use anyhow::{Context, bail};
use regex::Regex;

/// Numeric part of `TASK-<n>`.
pub fn task_number(task: &str) -> Option<u32> {
    let re = Regex::new(r"^TASK-0*([0-9]+)$").ok()?;
    re.captures(task)?.get(1)?.as_str().parse().ok()
}

/// `TASK-<n>` with at least three digits (matches the existing dir names).
pub fn task_id(n: u32) -> String {
    format!("TASK-{n:03}")
}

/// Every task id present under `tasks_dir`, ordered by number.
pub fn available(tasks_dir: &Path) -> anyhow::Result<Vec<String>> {
    let mut out: Vec<(u32, String)> = std::fs::read_dir(tasks_dir)
        .with_context(|| format!("cannot read {}", tasks_dir.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && path.join("README.md").is_file())
        .filter_map(|path| {
            let name = path.file_name()?.to_string_lossy().to_string();
            task_number(&name).map(|n| (n, name))
        })
        .collect();
    out.sort_by_key(|(n, _)| *n);
    Ok(out.into_iter().map(|(_, name)| name).collect())
}

/// Resolve a selection to ordered, deduplicated task ids. `all` expands to every task under
/// `tasks_dir`; numeric tokens are padded to the `TASK-NNN` shape.
pub fn resolve(tokens: &[String], tasks_dir: &Path) -> anyhow::Result<Vec<String>> {
    let range = Regex::new(r"^(?:TASK-0*([0-9]+)|([0-9]+))\s*\.\.\s*(?:TASK-0*([0-9]+)|([0-9]+))$")
        .expect("static regex");
    let dash = Regex::new(r"^(?:TASK-0*([0-9]+)|([0-9]+))\s*-\s*(?:TASK-0*([0-9]+)|([0-9]+))$")
        .expect("static regex");
    let single = Regex::new(r"^(?:TASK-0*([0-9]+)|([0-9]+))$").expect("static regex");
    let existing = available(tasks_dir)?;
    let existing_numbers: Vec<u32> = existing.iter().filter_map(|id| task_number(id)).collect();

    let mut picked: Vec<u32> = Vec::new();
    for token in tokens {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        // ids are matched case-insensitively: `task-101` and `TASK-101` are the same task
        let token = &token.to_ascii_uppercase();
        if token == "ALL" {
            picked.extend(existing_numbers.iter().copied());
            continue;
        }
        // `TASK-…`-prefixed forms are exact task numbers
        if let Some(caps) = range
            .captures(token)
            .filter(|caps| caps.get(1).is_some() && caps.get(3).is_some())
        {
            picked.extend(expand_range(
                first_number(&caps, 1, 2),
                first_number(&caps, 3, 4),
            )?);
            continue;
        }
        if let Some(caps) = dash
            .captures(token)
            .filter(|caps| caps.get(1).is_some() && caps.get(3).is_some())
        {
            picked.extend(expand_range(
                first_number(&caps, 1, 2),
                first_number(&caps, 3, 4),
            )?);
            continue;
        }
        if let Some(caps) = single.captures(token).filter(|caps| caps.get(1).is_some()) {
            picked.push(first_number(&caps, 1, 2));
            continue;
        }
        // bare numbers and bare-number ranges: `1-3` means the first three available tasks
        if let Some(caps) = range.captures(token) {
            picked.extend(bare_range(
                &existing_numbers,
                &existing,
                first_number(&caps, 1, 2),
                first_number(&caps, 3, 4),
            )?);
            continue;
        }
        if let Some(caps) = dash.captures(token) {
            picked.extend(bare_range(
                &existing_numbers,
                &existing,
                first_number(&caps, 1, 2),
                first_number(&caps, 3, 4),
            )?);
            continue;
        }
        if let Some(caps) = single.captures(token) {
            picked.push(bare_number(
                &existing_numbers,
                &existing,
                first_number(&caps, 1, 2),
            )?);
            continue;
        }
        bail!(
            "cannot parse task selection token {token:?}: use \"all\", \"1-3\", \"TASK-002..TASK-004\" or \"TASK-101\""
        );
    }

    picked.sort_unstable();
    picked.dedup();
    Ok(picked.into_iter().map(task_id).collect())
}

/// A bare number is a task number when that task exists, else a 1-based position in the list.
fn bare_number(existing_numbers: &[u32], existing: &[String], n: u32) -> anyhow::Result<u32> {
    if existing_numbers.contains(&n) {
        return Ok(n);
    }
    let index = usize::try_from(n).ok().and_then(|n| n.checked_sub(1));
    match index.and_then(|i| existing.get(i).map(|id| task_number(id)).unwrap_or(None)) {
        Some(number) => Ok(number),
        None => bail!("no task number {n} and no {n}-th task"),
    }
}

fn bare_range(
    existing_numbers: &[u32],
    existing: &[String],
    start: u32,
    end: u32,
) -> anyhow::Result<Vec<u32>> {
    if existing_numbers.contains(&start) && existing_numbers.contains(&end) {
        return expand_range(start, end);
    }
    let mut out = Vec::new();
    for n in start..=end {
        out.push(bare_number(existing_numbers, existing, n)?);
    }
    Ok(out)
}

fn first_number(caps: &regex::Captures, group_a: usize, group_b: usize) -> u32 {
    caps.get(group_a)
        .or_else(|| caps.get(group_b))
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0)
}

fn expand_range(start: u32, end: u32) -> anyhow::Result<Vec<u32>> {
    if end < start {
        bail!("selection range {start}..{end} is reversed");
    }
    if end - start > 10_000 {
        bail!("selection range {start}..{end} is unreasonably large");
    }
    Ok((start..=end).collect())
}

/// Drop ids already present in `done` (what `experiment --resume` uses).
pub fn skip_completed(all: &[String], done: &[String]) -> Vec<String> {
    all.iter()
        .filter(|id| !done.contains(id))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tasks_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for n in [101u32, 102, 103, 104, 105, 106] {
            let path = dir.path().join(task_id(n));
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("README.md"), "x").unwrap();
        }
        dir
    }

    #[test]
    fn single_id_and_number_normalize() {
        let dir = tasks_dir();
        assert_eq!(
            resolve(&["TASK-101".into()], dir.path()).unwrap(),
            vec!["TASK-101"]
        );
        assert_eq!(
            resolve(&["101".into()], dir.path()).unwrap(),
            vec!["TASK-101"]
        );
        assert_eq!(
            resolve(&["task-101".into()], dir.path()).unwrap(),
            vec!["TASK-101"]
        );
    }

    #[test]
    fn ranges_dedupe_and_order() {
        let dir = tasks_dir();
        let ids = resolve(
            &["TASK-103".into(), "1-3".into(), "TASK-102..TASK-104".into()],
            dir.path(),
        )
        .unwrap();
        assert_eq!(ids, vec!["TASK-101", "TASK-102", "TASK-103", "TASK-104"]);
    }

    #[test]
    fn all_expands_to_every_package_in_order() {
        let dir = tasks_dir();
        assert_eq!(
            resolve(&["all".into()], dir.path()).unwrap(),
            vec![
                "TASK-101", "TASK-102", "TASK-103", "TASK-104", "TASK-105", "TASK-106"
            ]
        );
    }

    #[test]
    fn bad_tokens_are_rejected() {
        let dir = tasks_dir();
        assert!(resolve(&["TASK-2..TASK-1".into()], dir.path()).is_err());
        assert!(resolve(&["nonsense".into()], dir.path()).is_err());
    }

    #[test]
    fn resume_skip_removes_finished_tasks() {
        let all = vec!["TASK-101".to_string(), "TASK-102".to_string()];
        assert_eq!(
            skip_completed(&all, &["TASK-101".to_string()]),
            vec!["TASK-102"]
        );
        assert_eq!(skip_completed(&all, &[]), all);
    }

    #[test]
    fn numbers_round_trip() {
        assert_eq!(task_number("TASK-007"), Some(7));
        assert_eq!(task_number("TASK-101"), Some(101));
        assert_eq!(task_number("task-101"), None);
        assert_eq!(task_id(7), "TASK-007");
    }
}
