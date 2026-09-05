//! The completion gate — a faithful port of `reference/task-template/verify.sh`, driven by the
//! declarative `verify.toml` instead of a sourced bash config.
//!
//! Contract: exit 0 AND last stdout line exactly `DONE` <=> every check passed. All checks run (no
//! short-circuit) unless `--fail-fast`. Any internal error yields `RESULT FAIL` + exit 70 — never a
//! silent `DONE`.

use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;
use serde::Serialize;

use crate::ops;
use crate::progress::ProgressFile;
use crate::taskfile::{self};
use crate::verifycfg::{self, FILE_NAME};

/// Exit code of a fully passing gate.
pub const EXIT_PASS: i32 = 0;
/// Exit code when at least one check failed.
pub const EXIT_FAIL: i32 = 1;
/// Exit code for a missing/unreadable config.
pub const EXIT_CONFIG: i32 = 2;
/// Exit code for an internal error (the ERR trap in the shell original).
pub const EXIT_INTERNAL: i32 = 70;

pub const DEFAULT_BASE: &str = "baseline";

#[derive(Debug, Clone)]
pub struct GateOpts {
    /// Repository root the gate runs in.
    pub root: PathBuf,
    /// Directory holding `README.md` + `verify.toml` (the trusted copies on the host).
    pub task_dir: PathBuf,
    /// Progress file; `None` or `Some("")` disables the progress check.
    pub progress: Option<String>,
    /// Explicit base ref (highest precedence).
    pub base: Option<String>,
    /// Directory for per-check logs; a fresh temp dir when `None`.
    pub log_dir: Option<PathBuf>,
    /// Stop at the first failing check.
    pub fail_fast: bool,
    /// Enforce the complete task-package lint contract before verification.
    pub enforce_task_contract: bool,
}

#[derive(Debug, Clone)]
pub struct GateOutput {
    pub exit: i32,
    /// Full stdout the gate produced (already scrubbed).
    pub text: String,
    pub last_line: String,
    pub summary: String,
    pub log_dir: PathBuf,
    /// Failing check names, in gate order.
    pub failed_checks: Vec<String>,
    /// Every check that ran, in gate order, with its verdict (structured: no parsing of `text`).
    pub checks: Vec<CheckResult>,
}

/// One named check and its verdict, e.g. `focused.1` / `regression.2` / `scope`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CheckResult {
    pub name: String,
    pub pass: bool,
    /// Gate verdict code: 0 on PASS. Command evidence retains its actual exit code.
    pub rc: i32,
    /// Stable, scrubbed command evidence. Built-in checks have no command evidence.
    pub evidence: Option<CommandEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandEvidence {
    pub exit: i32,
    pub stdout: String,
    pub stderr: String,
    pub matchers: Vec<MatcherResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatcherResult {
    pub kind: String,
    pub expected: String,
    pub actual: String,
    pub pass: bool,
}

/// Canonical, timestamp-free evidence sidecar for a gate run.
pub fn evidence_json(checks: &[CheckResult]) -> anyhow::Result<Vec<u8>> {
    #[derive(Serialize)]
    struct Evidence<'a> {
        schema: &'static str,
        checks: &'a [CheckResult],
    }
    Ok(serde_json::to_vec_pretty(&Evidence {
        schema: "gate-evidence/v1",
        checks,
    })?)
}

impl GateOutput {
    /// Verdict of one check by name; `None` when it never ran.
    pub fn check(&self, name: &str) -> Option<bool> {
        self.check_result(name).map(|check| check.pass)
    }

    /// The full result of one check by name; `None` when it never ran.
    pub fn check_result(&self, name: &str) -> Option<&CheckResult> {
        self.checks.iter().find(|check| check.name == name)
    }
}

impl GateOutput {
    /// The only pass signal: exit 0 and the last stdout line is exactly `DONE`.
    pub fn is_pass(&self) -> bool {
        self.exit == EXIT_PASS && self.last_line == "DONE"
    }
}

/// Run the gate. Gate-internal problems never produce a silent `DONE`: they become `RESULT FAIL`
/// with exit 70.
pub fn run(opts: GateOpts) -> GateOutput {
    match run_inner(opts) {
        Ok(output) => output,
        Err(err) => {
            let text = format!("RESULT FAIL internal-error {err:#}\nRESULT FAIL\n");
            GateOutput {
                exit: EXIT_INTERNAL,
                last_line: "RESULT FAIL".to_string(),
                summary: format!("RESULT FAIL internal-error {err:#}"),
                log_dir: PathBuf::new(),
                failed_checks: vec!["internal-error".to_string()],
                checks: Vec::new(),
                text,
            }
        }
    }
}

/// A check body reports its lines and either succeeds (`Ok`) or fails with a return code.
type CheckBody = Result<Vec<String>, (Vec<String>, i32)>;

struct Session {
    lines: Vec<String>,
    passes: usize,
    fails: usize,
    failed_checks: Vec<String>,
    checks: Vec<CheckResult>,
    log_dir: PathBuf,
    fail_fast: bool,
    halted: bool,
    root: PathBuf,
}

impl Session {
    fn log_path(&self, name: &str) -> PathBuf {
        self.log_dir.join(format!("{name}.log"))
    }

    /// Run one named check. Returns `false` when a `--fail-fast` run must stop.
    fn check(&mut self, name: &str, body: impl FnOnce() -> CheckBody) -> bool {
        self.record(name, body(), None)
    }

    fn record(&mut self, name: &str, body: CheckBody, evidence: Option<CommandEvidence>) -> bool {
        if self.halted {
            return false;
        }
        let (report, rc) = match body {
            Ok(lines) => (lines, 0),
            Err((lines, rc)) => (lines, rc),
        };
        let mut log = report.join("\n");
        log.push('\n');
        let log_path = self.log_path(name);
        if let Err(err) = crate::redact::write_scrubbed(&log_path, log.as_bytes()) {
            self.lines
                .push(format!("WARN log write failed for {name}: {err}"));
        }
        self.checks.push(CheckResult {
            name: name.to_string(),
            pass: rc == 0,
            rc,
            evidence,
        });
        if rc == 0 {
            self.lines.push(format!("CHECK {name} PASS"));
            self.passes += 1;
            true
        } else {
            self.lines.push(format!(
                "CHECK {name} FAIL rc={rc} log={}",
                log_path.display()
            ));
            self.fails += 1;
            self.failed_checks.push(name.to_string());
            let total = log.lines().count();
            self.lines
                .push(format!("--- tail {} ---", log_path.display()));
            for line in log.lines().skip(total.saturating_sub(40)) {
                self.lines
                    .push(format!("  | {}", crate::redact::scrub(line)));
            }
            self.lines.push("---".to_string());
            self.halted = self.fail_fast;
            !self.halted
        }
    }

    fn run_checks(&mut self, checks: &[verifycfg::Check]) {
        for check in checks {
            let name = check.id.clone();
            let root = self.root.clone();
            let check = check.clone();
            let (body, evidence) = run_configured_check(&root, &check);
            if !self.record(&name, body, Some(evidence)) {
                return;
            }
        }
    }

    fn finish(mut self) -> GateOutput {
        let summary = format!(
            "SUMMARY pass={} fail={} log_dir={}",
            self.passes,
            self.fails,
            self.log_dir.display()
        );
        self.lines.push(summary.clone());
        let (exit, last) = if self.fails == 0 {
            self.lines.push("RESULT PASS".to_string());
            self.lines.push("DONE".to_string());
            (EXIT_PASS, "DONE".to_string())
        } else {
            self.lines.push("RESULT FAIL".to_string());
            (EXIT_FAIL, "RESULT FAIL".to_string())
        };
        GateOutput {
            exit,
            last_line: last,
            summary,
            log_dir: self.log_dir,
            failed_checks: self.failed_checks,
            checks: self.checks,
            text: crate::redact::scrub(&(self.lines.join("\n") + "\n")),
        }
    }
}

fn run_inner(opts: GateOpts) -> anyhow::Result<GateOutput> {
    let progress_opt = opts.progress.clone().filter(|p| !p.is_empty());
    let root = opts.root.clone();
    let task_dir = opts.task_dir.clone();
    let verify_config = task_dir.join(FILE_NAME);
    let task_file = task_dir.join("README.md");

    let log_dir = match opts.log_dir.clone() {
        Some(dir) => {
            std::fs::create_dir_all(&dir)?;
            dir
        }
        None => tempfile::Builder::new().prefix("verify.").tempdir()?.keep(),
    };

    let mut session = Session {
        lines: Vec::new(),
        passes: 0,
        fails: 0,
        failed_checks: Vec::new(),
        checks: Vec::new(),
        log_dir,
        fail_fast: opts.fail_fast,
        halted: false,
        root: root.clone(),
    };

    // ---------- config ----------
    if !verify_config.is_file() {
        let text = format!(
            "CHECK config FAIL missing {}\nRESULT FAIL\n",
            verify_config.display()
        );
        return Ok(GateOutput {
            exit: EXIT_CONFIG,
            last_line: "RESULT FAIL".to_string(),
            summary: format!("CHECK config FAIL missing {}", verify_config.display()),
            log_dir: session.log_dir,
            failed_checks: vec!["config".to_string()],
            checks: vec![CheckResult {
                name: "config".to_string(),
                pass: false,
                rc: EXIT_FAIL,
                evidence: None,
            }],
            text,
        });
    }
    let cfg = verifycfg::VerifyConfig::load(&verify_config)?;
    session
        .lines
        .push(format!("CHECK config PASS {}", verify_config.display()));
    session.checks.push(CheckResult {
        name: "config".to_string(),
        pass: true,
        rc: 0,
        evidence: None,
    });

    // Direct gate enforces the same complete package contract as dispatch lint.
    if opts.enforce_task_contract {
        let lint_task_dir = task_dir.clone();
        session.check("task_lint", move || {
            let report = crate::lint::lint_path(&lint_task_dir);
            if report.passed() {
                Ok(report.render().lines().map(str::to_string).collect())
            } else {
                Err((report.render().lines().map(str::to_string).collect(), 1))
            }
        });
    }

    // ---------- scope ----------
    let base = resolve_base(&opts.base, &cfg)?;
    let scope_root = root.clone();
    let scope_base = base.clone();
    let scope_globs = cfg.writable_paths.clone();
    session.check("scope", move || {
        check_scope(&scope_root, &scope_base, &scope_globs)
    });

    // ---------- forbidden paths and patterns ----------
    let forb_root = root.clone();
    let forb_base = base.clone();
    let forbidden = cfg.forbidden_paths.clone();
    session.check("forbidden_paths", move || {
        check_forbidden_paths(&forb_root, &forb_base, &forbidden)
    });

    let pat_root = root.clone();
    let patterns = cfg.forbidden_patterns.clone();
    session.check("forbidden_patterns", move || {
        check_forbidden_patterns(&pat_root, &patterns)
    });

    // ---------- ordered stable checks ----------
    session.run_checks(&cfg.checks);

    // ---------- progress ----------
    if let Some(progress_path) = progress_opt {
        let progress_path = PathBuf::from(progress_path);
        let task_file = task_file.clone();
        session.check("progress", move || {
            check_progress(&task_file, &progress_path)
        });
    }

    Ok(session.finish())
}

/// `--base` > `TASKFMT_BASE` > an optional immutable standalone verifier base.
pub fn resolve_base(
    explicit: &Option<String>,
    cfg: &verifycfg::VerifyConfig,
) -> anyhow::Result<String> {
    let env = std::env::var("TASKFMT_BASE").ok();
    resolve_base_from(explicit, env.as_deref(), cfg)
}

/// Pure core of `resolve_base` (testable without touching process state).
pub fn resolve_base_from(
    explicit: &Option<String>,
    env: Option<&str>,
    cfg: &verifycfg::VerifyConfig,
) -> anyhow::Result<String> {
    if let Some(base) = explicit {
        return Ok(base.clone());
    }
    if let Some(env_base) = env.filter(|b| !b.is_empty()) {
        return Ok(env_base.to_string());
    }
    cfg.base_tree.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "no immutable base: pass --base or TASKFMT_BASE; lifecycle supplies its recorded base"
        )
    })
}

fn fail(lines: Vec<String>) -> CheckBody {
    Err((lines, 1))
}

fn check_scope(root: &Path, base: &str, globs: &[String]) -> CheckBody {
    let mut lines = Vec::new();
    if base.is_empty() {
        return fail(vec!["BASE_REF not set".to_string()]);
    }
    if !ref_resolves(root, base) {
        return fail(vec![format!("BASE_REF not resolvable: {base}")]);
    }
    if globs.is_empty() {
        return fail(vec!["ALLOWED_GLOBS empty".to_string()]);
    }
    let matchers: Vec<Regex> = globs.iter().filter_map(|g| glob_regex(g).ok()).collect();
    let mut rc = 0;
    // index flags that make `git diff` blind to worktree edits: reported, never trusted
    let hidden = match ops::git::hidden_index_entries(root) {
        Ok(entries) => entries,
        Err(err) => return fail(vec![format!("git ls-files -v failed: {err:#}")]),
    };
    if !hidden.is_empty() {
        lines.push(
            "HIDDEN index entries (skip-worktree 'S' / assume-unchanged lowercase) blind the diff:"
                .to_string(),
        );
        lines.extend(hidden);
        rc = 1;
    }
    let files = match ops::git::changed_files(root, base) {
        Ok(files) => files,
        Err(err) => return fail(vec![format!("git diff failed: {err:#}")]),
    };
    for file in files {
        if matchers.iter().any(|re| re.is_match(&file)) {
            lines.push(format!("ok      {file}"));
        } else {
            lines.push(format!("OUTSIDE {file}"));
            rc = 1;
        }
    }
    if rc == 0 { Ok(lines) } else { Err((lines, rc)) }
}

fn ref_resolves(root: &Path, base: &str) -> bool {
    ops::git::output(Command::new("git").current_dir(root).args([
        "rev-parse",
        "--verify",
        "--quiet",
        &format!("{base}^{{commit}}"),
    ]))
    .is_ok()
}

/// Bash-style glob where `*` and `**` both cross `/`: everything except `*` and `?` is literal.
fn glob_regex(glob: &str) -> anyhow::Result<Regex> {
    let mut pattern = String::from("^");
    for ch in glob.chars() {
        match ch {
            '*' => pattern.push_str(".*"),
            '?' => pattern.push('.'),
            other => pattern.push_str(&regex::escape(&other.to_string())),
        }
    }
    pattern.push('$');
    Ok(Regex::new(&pattern)?)
}

/// A forbidden path must not be created or modified by this run: it must be absent from the
/// changed-file set vs the base commit. Trusted material shipped by earlier tasks exists on the
/// base commit, so existence alone is not a violation — changing it is.
fn check_forbidden_paths(root: &Path, base: &str, paths: &[String]) -> CheckBody {
    let mut lines = Vec::new();
    let changed = match ops::git::changed_files(root, base) {
        Ok(files) => files,
        Err(e) => {
            return Err((vec![format!("BASE_UNRESOLABLE {base}: {e}")], 1));
        }
    };
    let mut rc = 0;
    for p in paths {
        if let Err(err) = confined_path(root, p) {
            lines.push(format!("UNSAFE {p}: {err}"));
            rc = 1;
            continue;
        }
        let dir_prefix = format!("{p}/");
        if changed.iter().any(|f| f == p || f.starts_with(&dir_prefix)) {
            lines.push(format!("CHANGED {p}"));
            rc = 1;
        } else {
            lines.push(format!("ok untouched {p}"));
        }
    }
    if rc == 0 { Ok(lines) } else { Err((lines, rc)) }
}

fn check_forbidden_patterns(root: &Path, patterns: &[verifycfg::ForbiddenPattern]) -> CheckBody {
    let mut lines = Vec::new();
    let mut rc = 0;
    for entry in patterns {
        let scopes: Vec<String> = if entry.paths.is_empty() {
            vec![".".to_string()]
        } else {
            entry.paths.clone()
        };
        for scope in &scopes {
            if let Err(err) = confined_path(root, scope) {
                lines.push(format!("UNSAFE {scope}: {err}"));
                rc = 1;
            }
        }
        if rc != 0 {
            continue;
        }
        let mut cmd = Command::new("grep");
        cmd.current_dir(root)
            .args(["-rIEn", "--exclude-dir=.git", "-e", &entry.regex, "--"]);
        cmd.args(&scopes);
        match ops::capture(&mut cmd) {
            Ok(captured) if captured.status == 1 => {
                lines.push(format!("ok /{}/ absent", entry.regex));
            }
            Ok(captured) if captured.status == 0 => {
                lines.push(format!("FORBIDDEN /{}/ found:", entry.regex));
                lines.extend(captured.stdout.trim_end().lines().map(str::to_string));
                rc = 1;
            }
            Ok(captured) => {
                lines.push(format!(
                    "pattern check /{}/ failed rc={}",
                    entry.regex, captured.status
                ));
                lines.extend(captured.stderr.trim_end().lines().map(str::to_string));
                rc = 1;
            }
            Err(err) => {
                lines.push(format!(
                    "pattern check /{}/ failed to start: {err}",
                    entry.regex
                ));
                rc = 1;
            }
        }
    }
    if rc == 0 { Ok(lines) } else { Err((lines, rc)) }
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
        anyhow::bail!("resolves outside repository");
    }
    Ok(candidate)
}

fn run_configured_check(root: &Path, check: &verifycfg::Check) -> (CheckBody, CommandEvidence) {
    let mut command = match (&check.argv, &check.shell) {
        (Some(argv), None) => {
            let mut c = Command::new(&argv[0]);
            c.current_dir(root).args(&argv[1..]);
            c
        }
        (None, Some(shell)) => {
            let mut c = Command::new("bash");
            c.current_dir(root).args(["-eo", "pipefail", "-c", shell]);
            c
        }
        _ => {
            return (
                fail(vec![format!("invalid command definition {}", check.id)]),
                CommandEvidence {
                    exit: 127,
                    stdout: String::new(),
                    stderr: String::new(),
                    matchers: vec![],
                },
            );
        }
    };
    let captured = match ops::capture(&mut command) {
        Ok(out) => out,
        Err(err) => {
            let evidence = CommandEvidence {
                exit: 127,
                stdout: String::new(),
                stderr: crate::redact::scrub(&err.to_string()),
                matchers: vec![MatcherResult {
                    kind: "spawn".into(),
                    expected: "command starts".into(),
                    actual: "spawn error".into(),
                    pass: false,
                }],
            };
            return (
                Err((
                    vec![format!("command {} failed to start: {err}", check.id)],
                    127,
                )),
                evidence,
            );
        }
    };
    let evidence = evaluate_expected(root, check, &captured);
    let mut report = vec![format!("exit={}", evidence.exit)];
    report.extend(evidence.matchers.iter().map(|m| {
        format!(
            "matcher kind={} expected={:?} actual={:?} pass={}",
            m.kind, m.expected, m.actual, m.pass
        )
    }));
    if !evidence.stdout.is_empty() {
        report.push(format!("stdout:\n{}", evidence.stdout));
    }
    if !evidence.stderr.is_empty() {
        report.push(format!("stderr:\n{}", evidence.stderr));
    }
    let pass = evidence.matchers.iter().all(|m| m.pass);
    (
        if pass {
            Ok(report)
        } else {
            // A zero process status can still fail its declared expectation.
            Err((
                report,
                if captured.status == 0 {
                    1
                } else {
                    captured.status
                },
            ))
        },
        evidence,
    )
}

fn evaluate_expected(
    root: &Path,
    check: &verifycfg::Check,
    captured: &ops::Captured,
) -> CommandEvidence {
    let expected = &check.expected;
    let stdout = crate::redact::scrub(&captured.stdout);
    let stderr = crate::redact::scrub(&captured.stderr);
    let mut matchers = Vec::new();
    let want_exit = expected.exit.unwrap_or(0);
    push_match(
        &mut matchers,
        "exit",
        want_exit.to_string(),
        captured.status.to_string(),
        captured.status == want_exit,
    );
    stream_matchers(
        &mut matchers,
        "stdout",
        &captured.stdout,
        &expected.stdout_contains,
        &expected.stdout_excludes,
        &expected.stdout_regex,
        &expected.stdout_occurrences,
    );
    stream_matchers(
        &mut matchers,
        "stderr",
        &captured.stderr,
        &expected.stderr_contains,
        &expected.stderr_excludes,
        &expected.stderr_regex,
        &expected.stderr_occurrences,
    );
    for path in &expected.required_artifacts {
        artifact_match(root, path, true, &mut matchers);
    }
    for path in &expected.forbidden_artifacts {
        artifact_match(root, path, false, &mut matchers);
    }
    CommandEvidence {
        exit: captured.status,
        stdout,
        stderr,
        matchers,
    }
}

fn push_match(
    out: &mut Vec<MatcherResult>,
    kind: &str,
    expected: String,
    actual: String,
    pass: bool,
) {
    out.push(MatcherResult {
        kind: kind.into(),
        expected,
        actual,
        pass,
    });
}
fn stream_matchers(
    out: &mut Vec<MatcherResult>,
    stream: &str,
    value: &str,
    contains: &[String],
    excludes: &[String],
    regexes: &[String],
    occurrences: &[verifycfg::Occurrence],
) {
    for needle in contains {
        push_match(
            out,
            &format!("{stream}.contains"),
            needle.clone(),
            value.contains(needle).to_string(),
            value.contains(needle),
        );
    }
    for needle in excludes {
        push_match(
            out,
            &format!("{stream}.excludes"),
            needle.clone(),
            value.contains(needle).to_string(),
            !value.contains(needle),
        );
    }
    for pattern in regexes {
        let pass = Regex::new(pattern).is_ok_and(|re| re.is_match(value));
        push_match(
            out,
            &format!("{stream}.regex"),
            pattern.clone(),
            pass.to_string(),
            pass,
        );
    }
    for occurrence in occurrences {
        let got = value.matches(&occurrence.text).count();
        push_match(
            out,
            &format!("{stream}.occurrences"),
            format!("{}={}", occurrence.text, occurrence.count),
            got.to_string(),
            got == occurrence.count,
        );
    }
}
fn artifact_match(root: &Path, path: &str, required: bool, out: &mut Vec<MatcherResult>) {
    let candidate = match confined_path(root, path) {
        Ok(p) => p,
        Err(e) => {
            push_match(
                out,
                if required {
                    "artifact.required"
                } else {
                    "artifact.forbidden"
                },
                path.into(),
                format!("unreadable path: {e}"),
                false,
            );
            return;
        }
    };
    let state = match std::fs::metadata(&candidate) {
        Ok(meta) if meta.is_file() => "file".to_string(),
        Ok(_) => "not-file".to_string(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "missing".to_string(),
        Err(e) => format!("unreadable: {e}"),
    };
    let pass = if required {
        state == "file"
    } else {
        state == "missing"
    };
    push_match(
        out,
        if required {
            "artifact.required"
        } else {
            "artifact.forbidden"
        },
        path.into(),
        state,
        pass,
    );
}

/// Normalize a checklist block: every checkbox becomes `[ ]` (the only token that may differ).
pub fn checklist_normalized(text: &str) -> Vec<String> {
    let re = Regex::new(r"^( *)- \[[ x]\] ").expect("static regex");
    taskfile::checklist_block(text)
        .into_iter()
        .map(|line| {
            re.replace(&line, |caps: &regex::Captures| {
                format!("{}- [ ] ", &caps[1])
            })
            .to_string()
        })
        .collect()
}

/// Progress is coordination state only. It must be a complete, internally-consistent event
/// stream; verifier checks still run independently and are the only completion evidence.
fn check_progress(task_file: &Path, progress_file: &Path) -> CheckBody {
    if !progress_file.is_file() {
        return fail(vec![format!("missing {}", progress_file.display())]);
    }
    if !task_file.is_file() {
        return fail(vec![format!("missing {}", task_file.display())]);
    }
    let task = match taskfile::TaskFile::load(task_file) {
        Ok(task) => task,
        Err(err) => {
            return fail(vec![format!(
                "cannot parse {}: {err:#}",
                task_file.display()
            )]);
        }
    };
    let progress = match ProgressFile::load(progress_file, &task) {
        Ok(progress) => progress,
        Err(err) => return fail(vec![format!("progress parse failed: {err:#}")]),
    };
    if progress.state != crate::progress::State::Done {
        return fail(vec![format!(
            "state={} (want DONE)",
            progress.state.as_str()
        )]);
    }
    Ok(vec![format!(
        "ok task={} state=DONE events={}",
        progress.task, progress.latest_event
    )])
}

/// First capture group of the first matching line, or an empty string.
pub fn extract(text: &str, pattern: &str) -> String {
    let Ok(re) = Regex::new(pattern) else {
        return String::new();
    };
    for line in text.lines() {
        if let Some(m) = re.captures(line).and_then(|caps| caps.get(1)) {
            return m.as_str().to_string();
        }
    }
    String::new()
}

/// Minimal line diff: `- removed`, `+ added` (the shell original piped `diff`; the reader only
/// needs the offending lines).
#[cfg(test)]
fn unified_diff(expected: &[String], actual: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, line) in expected.iter().enumerate() {
        if actual.get(i) != Some(line) {
            out.push(format!("- {line}"));
        }
    }
    for (i, line) in actual.iter().enumerate() {
        if expected.get(i) != Some(line) {
            out.push(format!("+ {line}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_matching_crosses_slashes_for_star_and_globstar() {
        let cases = [
            ("src/*", "src/auth/session/rotate.rs", true),
            ("src/*", "tests/auth/x.rs", false),
            ("src/**/*.rs", "src/a/b/c.rs", true),
            ("Cargo.lock", "Cargo.lock", true),
            ("docs/*", "docs/sub/dir.md", true),
        ];
        for (glob, path, want) in cases {
            assert_eq!(
                glob_regex(glob).unwrap().is_match(path),
                want,
                "{glob} vs {path}"
            );
        }
    }

    #[test]
    fn base_resolution_order() {
        let cfg = verifycfg::VerifyConfig::parse(
            "schema = \"verify/v2\"\ntask_id = \"TASK-001\"\nbase_tree = \"0123456789012345678901234567890123456789\"\nwritable_paths = [\"src\"]\n[[checks]]\nid = \"CHK-001\"\nphase = \"gate\"\nargv = [\"true\"]\n",
        )
        .unwrap();
        assert_eq!(
            resolve_base_from(&Some("flag".into()), None, &cfg).unwrap(),
            "flag"
        );
        assert_eq!(
            resolve_base_from(&None, None, &cfg).unwrap(),
            "0123456789012345678901234567890123456789"
        );
        assert_eq!(
            resolve_base_from(&None, Some("fromenv"), &cfg).unwrap(),
            "fromenv"
        );
        assert_eq!(
            resolve_base_from(&None, Some(""), &cfg).unwrap(),
            "0123456789012345678901234567890123456789"
        );
    }

    #[test]
    fn checklist_normalization_keeps_only_the_token() {
        let text = "<!-- checklist:start -->\n- [x] **1** a\n    - [ ] **1.1** b\n<!-- checklist:end -->\n";
        assert_eq!(
            checklist_normalized(text),
            vec!["- [ ] **1** a", "    - [ ] **1.1** b"]
        );
    }

    #[test]
    fn extract_reads_the_first_match_only() {
        let text = "TASK: TASK-042\nTASK: TASK-043\n";
        assert_eq!(extract(text, r"^TASK: *(TASK-[0-9]+)"), "TASK-042");
    }

    #[test]
    fn diff_lists_offending_lines() {
        let expected = vec!["a".to_string(), "b".to_string()];
        let actual = vec!["a".to_string(), "b!".to_string(), "c".to_string()];
        let diff = unified_diff(&expected, &actual);
        assert_eq!(diff, vec!["- b", "+ b!", "+ c"]);
    }

    fn check(expected: &str) -> verifycfg::Check {
        verifycfg::VerifyConfig::parse(&format!(
            "schema = \"verify/v2\"\ntask_id = \"TASK-001\"\nbase_tree = \"0123456789012345678901234567890123456789\"\nwritable_paths = [\"src\"]\n[[checks]]\nid = \"CHK-001\"\nphase = \"gate\"\nargv = [\"true\"]\nexpected = {expected}\n"
        )).unwrap().checks.remove(0)
    }

    #[test]
    fn expected_matchers_fail_independently_and_record_complete_evidence() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("present"), "ok").unwrap();
        let captured = ops::Captured {
            status: 0,
            stdout: "alpha alpha\n".into(),
            stderr: "beta\n".into(),
        };
        let cases = [
            ("{ exit = 1 }", "exit"),
            ("{ stdout_contains = [\"missing\"] }", "stdout.contains"),
            ("{ stderr_contains = [\"missing\"] }", "stderr.contains"),
            (
                "{ stdout_occurrences = [{ text = \"alpha\", count = 1 }] }",
                "stdout.occurrences",
            ),
            (
                "{ required_artifacts = [\"missing\"] }",
                "artifact.required",
            ),
            (
                "{ forbidden_artifacts = [\"present\"] }",
                "artifact.forbidden",
            ),
        ];
        for (expected, kind) in cases {
            let evidence = evaluate_expected(root.path(), &check(expected), &captured);
            assert!(
                evidence.matchers.iter().any(|m| m.kind == kind && !m.pass),
                "{expected:?}: {evidence:?}"
            );
            assert_eq!(evidence.exit, 0);
            assert_eq!(evidence.stdout, "alpha alpha\n");
            assert_eq!(evidence.stderr, "beta\n");
        }
    }

    #[test]
    fn invalid_matcher_definition_is_rejected_before_execution() {
        let err = verifycfg::VerifyConfig::parse(
            "schema = \"verify/v2\"\ntask_id = \"TASK-001\"\nbase_tree = \"0123456789012345678901234567890123456789\"\nwritable_paths = [\"src\"]\n[[checks]]\nid = \"CHK-001\"\nphase = \"gate\"\nargv = [\"true\"]\nexpected = { stdout_regex = [\"(\"] }\n",
        ).unwrap_err().to_string();
        assert!(err.contains("invalid regex"), "{err}");
    }

    #[test]
    fn configured_command_runs_once_and_mismatch_fails_after_zero_exit() {
        let root = tempfile::tempdir().unwrap();
        let check = verifycfg::VerifyConfig::parse(
            "schema = \"verify/v2\"\ntask_id = \"TASK-001\"\nbase_tree = \"0123456789012345678901234567890123456789\"\nwritable_paths = [\"src\"]\n[[checks]]\nid = \"CHK-001\"\nphase = \"gate\"\nshell = \"printf x >> invoked; printf actual\"\nexpected = { stdout_contains = [\"impossible\"] }\n",
        ).unwrap().checks.into_iter().next().unwrap();
        let (body, evidence) = run_configured_check(root.path(), &check);
        assert!(body.is_err());
        assert_eq!(
            std::fs::read_to_string(root.path().join("invoked")).unwrap(),
            "x"
        );
        assert_eq!(evidence.stdout, "actual");
        assert!(
            evidence
                .matchers
                .iter()
                .any(|m| m.kind == "stdout.contains" && !m.pass)
        );
    }
}
