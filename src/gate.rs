//! Local task verification: package lint, scope/invariant checks, declared commands, progress.

use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;

use crate::ops;
use crate::progress::{ProgressFile, State};
use crate::taskfile::TaskFile;
use crate::verifycfg::{self, FILE_NAME};

const EXIT_PASS: i32 = 0;
const EXIT_FAIL: i32 = 1;
const EXIT_INTERNAL: i32 = 70;

#[derive(Debug, Clone)]
pub struct GateOpts {
    pub root: PathBuf,
    pub task_dir: PathBuf,
    /// `None` runs checks only. `Some(path)` requires a completed progress file for `DONE`.
    pub progress: Option<PathBuf>,
    pub base: String,
}

#[derive(Debug, Clone)]
pub struct GateOutput {
    pub exit: i32,
    pub text: String,
}

type CheckBody = Result<Vec<String>, Vec<String>>;

struct Session {
    lines: Vec<String>,
    passes: usize,
    failures: usize,
}

impl Session {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            passes: 0,
            failures: 0,
        }
    }

    fn check(&mut self, name: &str, body: CheckBody) {
        match body {
            Ok(lines) => {
                self.lines.push(format!("CHECK {name} PASS"));
                self.lines
                    .extend(lines.into_iter().map(|line| format!("  {line}")));
                self.passes += 1;
            }
            Err(lines) => {
                self.lines.push(format!("CHECK {name} FAIL"));
                self.lines
                    .extend(lines.into_iter().map(|line| format!("  {line}")));
                self.failures += 1;
            }
        }
    }

    fn finish(mut self, checks_only: bool) -> GateOutput {
        let summary = format!("SUMMARY pass={} fail={}", self.passes, self.failures);
        self.lines.push(summary.clone());
        let exit = if self.failures != 0 {
            self.lines.push("RESULT FAIL".to_string());
            EXIT_FAIL
        } else if checks_only {
            self.lines.push("CHECKS PASS".to_string());
            EXIT_PASS
        } else {
            self.lines.push("RESULT PASS".to_string());
            self.lines.push("DONE".to_string());
            EXIT_PASS
        };
        GateOutput {
            exit,
            text: self.lines.join("\n") + "\n",
        }
    }
}

pub fn run(options: GateOpts) -> GateOutput {
    match run_inner(options) {
        Ok(output) => output,
        Err(error) => {
            let summary = format!("RESULT FAIL internal-error {error:#}");
            GateOutput {
                exit: EXIT_INTERNAL,
                text: format!("{summary}\nRESULT FAIL\n"),
            }
        }
    }
}

fn run_inner(options: GateOpts) -> anyhow::Result<GateOutput> {
    anyhow::ensure!(
        options.root.is_dir(),
        "workspace root is not a directory: {}",
        options.root.display()
    );
    anyhow::ensure!(
        options.task_dir.is_dir(),
        "task directory is not a directory: {}",
        options.task_dir.display()
    );
    anyhow::ensure!(!options.base.trim().is_empty(), "scope base is empty");

    let config_path = options.task_dir.join(FILE_NAME);
    let config = verifycfg::VerifyConfig::load(&config_path)
        .map_err(|error| anyhow::anyhow!("cannot load {}: {error:#}", config_path.display()))?;

    let mut session = Session::new();
    session.check("config", Ok(vec![format!("{}", config_path.display())]));

    let task_lint = crate::lint::lint_path(&options.task_dir);
    if task_lint.passed() {
        session.check("task_lint", Ok(Vec::new()));
    } else {
        session.check(
            "task_lint",
            Err(task_lint.render().lines().map(str::to_string).collect()),
        );
    }

    if let Some(base_tree) = &config.base_tree {
        session.check(
            "base_tree",
            check_base_tree(&options.root, &options.base, base_tree),
        );
    }

    let mut allowed = config.writable_paths.clone();
    if let Some(progress) = &options.progress
        && let Ok(relative) = progress.strip_prefix(&options.root)
        && let Some(relative) = relative.to_str()
    {
        allowed.push(relative.replace('\\', "/"));
    }
    session.check("scope", check_scope(&options.root, &options.base, &allowed));
    session.check(
        "forbidden_paths",
        check_forbidden_paths(&options.root, &options.base, &config.forbidden_paths),
    );
    session.check(
        "forbidden_patterns",
        check_forbidden_patterns(&options.root, &config.forbidden_patterns),
    );

    for check in &config.checks {
        session.check(&check.id, run_configured_check(&options.root, check));
    }

    if let Some(progress_path) = &options.progress {
        session.check(
            "progress",
            check_progress(&options.task_dir.join("README.md"), progress_path),
        );
    }
    Ok(session.finish(options.progress.is_none()))
}

fn check_scope(root: &Path, base: &str, globs: &[String]) -> CheckBody {
    if globs.is_empty() {
        return Err(vec!["writable_paths is empty".to_string()]);
    }
    if !ops::git::resolves_commit(root, base) {
        return Err(vec![format!(
            "base ref does not resolve to a commit: {base}"
        )]);
    }
    let matchers = match globs
        .iter()
        .map(|glob| glob_regex(glob))
        .collect::<anyhow::Result<Vec<_>>>()
    {
        Ok(matchers) => matchers,
        Err(error) => return Err(vec![format!("invalid writable_paths pattern: {error:#}")]),
    };
    let hidden = match ops::git::hidden_index_entries(root) {
        Ok(entries) => entries,
        Err(error) => return Err(vec![format!("cannot inspect Git index flags: {error:#}")]),
    };
    if !hidden.is_empty() {
        return Err(vec![
            "Git index has skip-worktree or assume-unchanged entries:".to_string(),
            hidden.join("\n"),
        ]);
    }
    let changed = match ops::git::changed_files(root, base) {
        Ok(files) => files,
        Err(error) => {
            return Err(vec![format!(
                "cannot list changes from base {base}: {error:#}"
            )]);
        }
    };
    let outside: Vec<_> = changed
        .into_iter()
        .filter(|path| !matchers.iter().any(|matcher| matcher.is_match(path)))
        .collect();
    if outside.is_empty() {
        Ok(Vec::new())
    } else {
        Err(outside
            .into_iter()
            .map(|path| format!("changed path is outside writable_paths: {path}"))
            .collect())
    }
}

fn check_base_tree(root: &Path, base: &str, base_tree: &str) -> CheckBody {
    let supplied = match ops::git::resolve_commit(root, base) {
        Ok(commit) => commit,
        Err(error) => {
            return Err(vec![format!(
                "--base does not resolve to a commit: {error:#}"
            )]);
        }
    };
    let pinned = match ops::git::resolve_commit(root, base_tree) {
        Ok(commit) => commit,
        Err(error) => {
            return Err(vec![format!(
                "base_tree does not resolve to a commit: {error:#}"
            )]);
        }
    };
    if supplied == pinned {
        Ok(vec![format!("--base resolves to pinned commit {pinned}")])
    } else {
        Err(vec![format!(
            "--base resolves to {supplied}, but base_tree pins {pinned}"
        )])
    }
}

/// `*` and `?` cross directory separators, matching the task/v5 glob behavior.
fn glob_regex(glob: &str) -> anyhow::Result<Regex> {
    let mut pattern = String::from("^");
    for character in glob.chars() {
        match character {
            '*' => pattern.push_str(".*"),
            '?' => pattern.push('.'),
            other => pattern.push_str(&regex::escape(&other.to_string())),
        }
    }
    if !glob.contains(['*', '?']) {
        pattern.push_str("(?:/.*)?");
    }
    pattern.push('$');
    Ok(Regex::new(&pattern)?)
}

fn check_forbidden_paths(root: &Path, base: &str, forbidden: &[String]) -> CheckBody {
    let changed = match ops::git::changed_files(root, base) {
        Ok(paths) => paths,
        Err(error) => {
            return Err(vec![format!(
                "cannot check forbidden paths against {base}: {error:#}"
            )]);
        }
    };
    let mut failures = Vec::new();
    for path in forbidden {
        if let Err(error) = confined_path(root, path) {
            failures.push(format!("unsafe forbidden path {path}: {error:#}"));
            continue;
        }
        let prefix = format!("{path}/");
        if changed
            .iter()
            .any(|changed| changed == path || changed.starts_with(&prefix))
        {
            failures.push(format!("forbidden path changed: {path}"));
        }
    }
    if failures.is_empty() {
        Ok(Vec::new())
    } else {
        Err(failures)
    }
}

fn check_forbidden_patterns(root: &Path, patterns: &[verifycfg::ForbiddenPattern]) -> CheckBody {
    let mut failures = Vec::new();
    for entry in patterns {
        let regex = match Regex::new(&entry.regex) {
            Ok(regex) => regex,
            Err(error) => {
                failures.push(format!(
                    "invalid forbidden pattern {:?}: {error}",
                    entry.regex
                ));
                continue;
            }
        };
        let scopes = if entry.paths.is_empty() {
            vec![".".to_string()]
        } else {
            entry.paths.clone()
        };
        for scope in scopes {
            let path = match confined_path(root, &scope) {
                Ok(path) => path,
                Err(error) => {
                    failures.push(format!("unsafe forbidden-pattern scope {scope}: {error:#}"));
                    continue;
                }
            };
            let mut files = Vec::new();
            if let Err(error) = collect_files(root, &path, &mut files) {
                failures.push(format!("cannot scan {scope}: {error:#}"));
                continue;
            }
            for file in files {
                match std::fs::read(&file) {
                    Ok(bytes) => {
                        if let Ok(text) = std::str::from_utf8(&bytes)
                            && regex.is_match(text)
                        {
                            let relative = file.strip_prefix(root).unwrap_or(&file);
                            failures.push(format!(
                                "forbidden pattern {:?} matched {}",
                                entry.regex,
                                relative.display()
                            ));
                        }
                    }
                    Err(error) => failures.push(format!("cannot read {}: {error}", file.display())),
                }
            }
        }
    }
    if failures.is_empty() {
        Ok(Vec::new())
    } else {
        Err(failures)
    }
}

fn collect_files(root: &Path, path: &Path, output: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        anyhow::bail!("symlink is not scanned: {}", path.display());
    }
    if metadata.is_file() {
        output.push(path.to_path_buf());
        return Ok(());
    }
    if !metadata.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_name() == ".git" {
            continue;
        }
        let child = entry.path();
        if !child.starts_with(root) {
            anyhow::bail!("path escaped workspace root: {}", child.display());
        }
        if entry.file_type()?.is_symlink() {
            continue;
        }
        collect_files(root, &child, output)?;
    }
    Ok(())
}

fn confined_path(root: &Path, relative: &str) -> anyhow::Result<PathBuf> {
    let canonical_root = root.canonicalize()?;
    let candidate = root.join(relative);
    let mut existing = candidate.as_path();
    while !existing.exists() {
        existing = existing
            .parent()
            .ok_or_else(|| anyhow::anyhow!("no existing parent"))?;
    }
    let resolved = existing.canonicalize()?;
    if !resolved.starts_with(&canonical_root) {
        anyhow::bail!("resolves outside workspace root");
    }
    Ok(candidate)
}

fn run_configured_check(root: &Path, check: &verifycfg::Check) -> CheckBody {
    let mut command = match (&check.argv, &check.shell) {
        (Some(argv), None) => {
            let mut command = Command::new(&argv[0]);
            command.current_dir(root).args(&argv[1..]);
            command
        }
        (None, Some(shell)) => {
            let mut command = Command::new("bash");
            command
                .current_dir(root)
                .args(["-eo", "pipefail", "-c", shell]);
            command
        }
        _ => return Err(vec![format!("{} needs exactly one command form", check.id)]),
    };
    let captured = match ops::capture(&mut command) {
        Ok(captured) => captured,
        Err(error) => return Err(vec![format!("{} could not start: {error}", check.id)]),
    };
    let failures = expected_failures(root, check, &captured);
    if failures.is_empty() {
        Ok(Vec::new())
    } else {
        let mut report = vec![format!("{} exited with {}", check.id, captured.status)];
        report.extend(failures);
        if !captured.stdout.is_empty() {
            report.push(format!("stdout: {}", captured.stdout.trim_end()));
        }
        if !captured.stderr.is_empty() {
            report.push(format!("stderr: {}", captured.stderr.trim_end()));
        }
        Err(report)
    }
}

fn expected_failures(
    root: &Path,
    check: &verifycfg::Check,
    captured: &ops::Captured,
) -> Vec<String> {
    let expected = &check.expected;
    let mut failures = Vec::new();
    let want_exit = expected.exit.unwrap_or(0);
    if captured.status != want_exit {
        failures.push(format!(
            "expected exit {want_exit}, got {}",
            captured.status
        ));
    }
    check_stream(
        &mut failures,
        &check.id,
        StreamExpectation {
            name: "stdout",
            value: &captured.stdout,
            contains: &expected.stdout_contains,
            excludes: &expected.stdout_excludes,
            patterns: &expected.stdout_regex,
            occurrences: &expected.stdout_occurrences,
        },
    );
    check_stream(
        &mut failures,
        &check.id,
        StreamExpectation {
            name: "stderr",
            value: &captured.stderr,
            contains: &expected.stderr_contains,
            excludes: &expected.stderr_excludes,
            patterns: &expected.stderr_regex,
            occurrences: &expected.stderr_occurrences,
        },
    );
    for path in &expected.required_artifacts {
        check_artifact(root, path, true, &mut failures);
    }
    for path in &expected.forbidden_artifacts {
        check_artifact(root, path, false, &mut failures);
    }
    failures
}

struct StreamExpectation<'a> {
    name: &'static str,
    value: &'a str,
    contains: &'a [String],
    excludes: &'a [String],
    patterns: &'a [String],
    occurrences: &'a [verifycfg::Occurrence],
}

fn check_stream(failures: &mut Vec<String>, check_id: &str, stream: StreamExpectation<'_>) {
    for needle in stream.contains {
        if !stream.value.contains(needle) {
            failures.push(format!("{check_id} {} lacks {needle:?}", stream.name));
        }
    }
    for needle in stream.excludes {
        if stream.value.contains(needle) {
            failures.push(format!(
                "{check_id} {} contains forbidden {needle:?}",
                stream.name
            ));
        }
    }
    for pattern in stream.patterns {
        let matches = Regex::new(pattern).is_ok_and(|regex| {
            regex.is_match(stream.value)
                || regex.is_match(stream.value.trim_end_matches(['\r', '\n']))
        });
        if !matches {
            failures.push(format!(
                "{check_id} {} does not match /{pattern}/",
                stream.name
            ));
        }
    }
    for occurrence in stream.occurrences {
        let actual = stream.value.matches(&occurrence.text).count();
        if actual != occurrence.count {
            failures.push(format!(
                "{check_id} {} expected {:?} {} times, got {actual}",
                stream.name, occurrence.text, occurrence.count
            ));
        }
    }
}

fn check_artifact(root: &Path, path: &str, required: bool, failures: &mut Vec<String>) {
    let candidate = match confined_path(root, path) {
        Ok(candidate) => candidate,
        Err(error) => {
            failures.push(format!("artifact {path} is unsafe: {error:#}"));
            return;
        }
    };
    let is_file = std::fs::metadata(&candidate).is_ok_and(|metadata| metadata.is_file());
    if required && !is_file {
        failures.push(format!(
            "required artifact is missing or not a file: {path}"
        ));
    } else if !required && is_file {
        failures.push(format!("forbidden artifact exists: {path}"));
    }
}

fn check_progress(task_file: &Path, progress_file: &Path) -> CheckBody {
    let task = match TaskFile::load(task_file) {
        Ok(task) => task,
        Err(error) => {
            return Err(vec![format!(
                "cannot read task {}: {error:#}",
                task_file.display()
            )]);
        }
    };
    let progress = match ProgressFile::load(progress_file, &task) {
        Ok(progress) => progress,
        Err(error) => {
            return Err(vec![format!(
                "invalid progress {}: {error:#}",
                progress_file.display()
            )]);
        }
    };
    if progress.state != State::Done {
        return Err(vec![format!(
            "progress state is {}, expected DONE",
            progress.state.as_str()
        )]);
    }
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writable_globs_cross_slashes_and_exact_paths_cover_directories() {
        for (glob, path, expected) in [
            ("src/*", "src/auth/session.rs", true),
            ("src/*", "tests/session.rs", false),
            ("Cargo.lock", "Cargo.lock", true),
            ("src", "src/lib.rs", true),
        ] {
            assert_eq!(glob_regex(glob).unwrap().is_match(path), expected);
        }
    }

    #[test]
    fn expected_regex_accepts_a_match_before_trailing_newlines() {
        let mut failures = Vec::new();
        check_stream(
            &mut failures,
            "CHK-001",
            StreamExpectation {
                name: "stdout",
                value: "complete\n",
                contains: &[],
                excludes: &[],
                patterns: &["^complete$".to_string()],
                occurrences: &[],
            },
        );
        assert!(failures.is_empty(), "{failures:?}");
    }
}
