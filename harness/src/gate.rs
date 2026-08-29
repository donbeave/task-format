//! The completion gate — a faithful port of `reference/task-template/verify.sh`, driven by the
//! declarative `verify.toml` instead of a sourced bash config.
//!
//! Contract: exit 0 AND last stdout line exactly `DONE` <=> every check passed. All checks run (no
//! short-circuit) unless `--fail-fast`. Any internal error yields `RESULT FAIL` + exit 70 — never a
//! silent `DONE`.

use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub name: String,
    pub pass: bool,
    /// Return code of the check: 0 on PASS; for command checks the shell's status (126 not
    /// executable, 127 not found — also used when the shell could not be spawned); 1 for a
    /// failing built-in check.
    pub rc: i32,
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
    root: PathBuf,
}

impl Session {
    fn log_path(&self, name: &str) -> PathBuf {
        self.log_dir.join(format!("{name}.log"))
    }

    /// Run one named check. Returns `false` when a `--fail-fast` run must stop.
    fn check(&mut self, name: &str, body: impl FnOnce() -> CheckBody) -> bool {
        let (report, rc) = match body() {
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
            !self.fail_fast
        }
    }

    /// Run a command list under `bash -eo pipefail`, naming checks `<prefix>.<n>`. Under
    /// errexit + pipefail a failing command or pipe stage aborts the check, except where bash
    /// swallows the status: a non-final member of a `&&`/`||` list, the condition of an
    /// `if`/`while`, or a `!`-inverted command. `false && echo A; echo B` prints B and exits 0.
    fn run_cmd_list(&mut self, prefix: &str, commands: &[String]) {
        for (i, cmd) in commands.iter().enumerate() {
            let name = format!("{prefix}.{}", i + 1);
            let root = self.root.clone();
            let owned = cmd.clone();
            if !self.check(&name, move || run_shell(&root, &owned)) {
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
    });

    // ---------- scope ----------
    let base = resolve_base(&opts.base, &cfg);
    let scope_root = root.clone();
    let scope_base = base.clone();
    let scope_globs = cfg.allowed_globs.clone();
    session.check("scope", move || {
        check_scope(&scope_root, &scope_base, &scope_globs)
    });

    // ---------- required / forbidden paths and patterns ----------
    let req_root = root.clone();
    let required = cfg.required_paths.clone();
    session.check("required_paths", move || {
        check_required_paths(&req_root, &required)
    });

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

    // ---------- command groups ----------
    session.run_cmd_list("focused", &cfg.focused.commands);
    session.run_cmd_list("regression", &cfg.regression.commands);
    session.run_cmd_list("lint", &cfg.lint.commands);

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

/// `--base` > `TASKFMT_BASE` > `base_ref` in verify.toml > `baseline`.
pub fn resolve_base(explicit: &Option<String>, cfg: &verifycfg::VerifyConfig) -> String {
    let env = std::env::var("TASKFMT_BASE").ok();
    resolve_base_from(explicit, env.as_deref(), cfg)
}

/// Pure core of `resolve_base` (testable without touching process state).
pub fn resolve_base_from(
    explicit: &Option<String>,
    env: Option<&str>,
    cfg: &verifycfg::VerifyConfig,
) -> String {
    if let Some(base) = explicit {
        return base.clone();
    }
    if let Some(env_base) = env.filter(|b| !b.is_empty()) {
        return env_base.to_string();
    }
    if let Some(base) = cfg.base_ref.as_deref().filter(|b| !b.is_empty()) {
        return base.to_string();
    }
    DEFAULT_BASE.to_string()
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

fn check_required_paths(root: &Path, paths: &[String]) -> CheckBody {
    let mut lines = Vec::new();
    let mut rc = 0;
    for p in paths {
        if root.join(p).exists() {
            lines.push(format!("ok {p}"));
        } else {
            lines.push(format!("MISSING {p}"));
            rc = 1;
        }
    }
    if rc == 0 { Ok(lines) } else { Err((lines, rc)) }
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
        let mut cmd = Command::new("grep");
        cmd.current_dir(root)
            .args(["-rIEn", "--exclude-dir=.git", "-e", &entry.regex, "--"]);
        cmd.args(&scopes);
        let captured = ops::capture(&mut cmd).unwrap_or_else(|_| ops::Captured::default());
        let hits = captured.stdout.trim_end().to_string();
        if hits.is_empty() {
            lines.push(format!("ok /{}/ absent", entry.regex));
        } else {
            lines.push(format!("FORBIDDEN /{}/ found:", entry.regex));
            lines.extend(hits.lines().map(str::to_string));
            rc = 1;
        }
    }
    if rc == 0 { Ok(lines) } else { Err((lines, rc)) }
}

fn run_shell(root: &Path, cmd: &str) -> CheckBody {
    let mut command = Command::new("bash");
    command
        .current_dir(root)
        .args(["-eo", "pipefail", "-c", cmd]);
    match ops::capture(&mut command) {
        Ok(out) => {
            let mut lines = Vec::new();
            if !out.stdout.trim_end().is_empty() {
                lines.push(out.stdout.trim_end().to_string());
            }
            if !out.stderr.trim_end().is_empty() {
                lines.push(out.stderr.trim_end().to_string());
            }
            if out.status == 0 {
                Ok(lines)
            } else {
                lines.insert(0, format!("command: {cmd}"));
                Err((lines, out.status))
            }
        }
        Err(err) => Err((vec![format!("command: {cmd}"), err.to_string()], 127)),
    }
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

/// The progress check, ported from `check_progress` in verify.sh.
fn check_progress(task_file: &Path, progress_file: &Path) -> CheckBody {
    let mut lines: Vec<String> = Vec::new();

    if !progress_file.is_file() {
        return fail(vec![format!("missing {}", progress_file.display())]);
    }
    if !task_file.is_file() {
        return fail(vec![format!("missing {}", task_file.display())]);
    }
    let task_text = match std::fs::read_to_string(task_file) {
        Ok(text) => text,
        Err(err) => return fail(vec![format!("cannot read {}: {err}", task_file.display())]),
    };
    let progress_text = match std::fs::read_to_string(progress_file) {
        Ok(text) => text,
        Err(err) => {
            return fail(vec![format!(
                "cannot read {}: {err}",
                progress_file.display()
            )]);
        }
    };

    let expected = checklist_normalized(&task_text);
    let actual = checklist_normalized(&progress_text);
    if expected.is_empty() {
        return fail(vec![format!(
            "no checklist block in {}",
            task_file.display()
        )]);
    }
    if expected != actual {
        let mut out = vec!["checklist text/structure differs from README.md:".to_string()];
        out.extend(unified_diff(&expected, &actual));
        return fail(out);
    }

    // structural analysis of the progress checklist: the raw lines, not the normalized copy —
    // normalization erases the very [x] token this check is judging
    let items = taskfile::parse_checklist(&taskfile::checklist_block(&progress_text));
    let leaves = taskfile::leaf_flags(&items);
    let mut bad: Vec<String> = Vec::new();
    let mut leaf_count = 0usize;
    let mut checked_count = 0usize;
    for (i, item) in items.iter().enumerate() {
        if !item.well_formed {
            bad.push(format!("BAD LINE: {}", item.raw));
            continue;
        }
        if item.id.split('.').count() != item.depth + 1 {
            bad.push(format!("DEPTH/ID MISMATCH: {}", item.raw));
        }
        if leaves[i] {
            leaf_count += 1;
            if item.checked {
                checked_count += 1;
            } else {
                bad.push(format!("UNCHECKED LEAF {}", item.id));
            }
        } else {
            let children: Vec<bool> = items[i + 1..]
                .iter()
                .take_while(|candidate| candidate.depth > item.depth)
                .map(|candidate| candidate.checked)
                .collect();
            let all_kids = children.iter().all(|checked| *checked);
            if item.checked && !all_kids {
                bad.push(format!("PARENT CHECKED WITH UNCHECKED CHILD {}", item.id));
            }
            if !item.checked && all_kids {
                bad.push(format!("PARENT UNCHECKED BUT CHILDREN DONE {}", item.id));
            }
        }
    }
    lines.push(format!("leaves={leaf_count} checked={checked_count}"));
    if !bad.is_empty() {
        return Err((bad, 1));
    }

    // header fields
    let task = extract(&progress_text, r"^TASK: *(TASK-[0-9]+)");
    let id = extract(&task_text, r#"^id: *"?(TASK-[0-9]+)"?"#);
    let state = extract(&progress_text, r"^STATE: *([A-Z_]+)");
    let current = extract(&progress_text, r"^CURRENT: *([0-9.]+|NONE)");
    let baseline = extract(&progress_text, r"^BASELINE: *(.*[^ ])");

    if task.is_empty() || task != id {
        return fail(vec![format!("TASK={task} (want {id} from README.md)")]);
    }
    if state != "DONE" {
        return fail(vec![format!("STATE={state} (want DONE)")]);
    }
    if current != "NONE" {
        return fail(vec![format!("CURRENT={current} (want NONE)")]);
    }
    if baseline.is_empty() || baseline == "<not run>" {
        return fail(vec![
            "BASELINE not recorded (want '<command> -> <observed result>')".to_string(),
        ]);
    }
    if let Err(err) = ProgressFile::parse(&progress_text) {
        return fail(vec![format!("progress parse failed: {err:#}")]);
    }
    lines.push(format!(
        "ok TASK={task} STATE=DONE CURRENT=NONE BASELINE recorded"
    ));
    Ok(lines)
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
        let cfg =
            verifycfg::VerifyConfig::parse("schema = \"verify/v1\"\nbase_ref = \"fromcfg\"\n")
                .unwrap();
        assert_eq!(resolve_base_from(&Some("flag".into()), None, &cfg), "flag");
        assert_eq!(resolve_base_from(&None, None, &cfg), "fromcfg");
        assert_eq!(resolve_base_from(&None, Some("fromenv"), &cfg), "fromenv");
        let none = verifycfg::VerifyConfig::parse("schema = \"verify/v1\"\n").unwrap();
        assert_eq!(resolve_base_from(&None, Some(""), &none), "baseline");
        assert_eq!(resolve_base_from(&None, None, &none), "baseline");
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
}
