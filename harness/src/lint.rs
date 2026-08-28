//! Task lint — a faithful port of `harness/task-lint.sh` (schema `task/v4`).
//!
//! Exit 0 = every rule passed. Exit 1 = at least one ERROR. Warnings never fail.
//! Output: one line per finding — `ERROR <rule>: <detail>` or `WARN  <rule>: <detail>` — then
//! `SUMMARY errors=N warnings=M` and `LINT PASS|FAIL`. Messages are byte-identical to the shell
//! original so existing fixtures and expectations keep working.
//!
//! Rules (see `harness/README.md` "Author checklist"):
//!
//! - `frontmatter`   schema task/v4; id TASK-<n>; kind in the allowed set; verify; expected_paths
//! - `sections`      every required H2 present, in template order
//! - `placeholders`  no template placeholders left. The `<...>` set is derived from the template
//!   README (`TASK_TEMPLATE_README`, else `reference/task-template/README.md` under the nearest
//!   ancestor of the cwd or the compile-time repo root, else the copy embedded at build time); a
//!   span counts only when it is bare prose or the entire content of an inline-code span —
//!   `<...>` inside a longer code span (`pgtui --db <path>`, `Vec<TableRef>`) is literal.
//!   TASK-000 / P-NNN / AC-NNN stay hand-listed. Bare spans with a space outside the set warn.
//! - `ids`           no duplicate P-/R-/D- definition or AC- row
//! - `context`       "Read before editing" list never references /task/ (binding docs are not hints)
//! - `baseline`      fenced Baseline command equals some AC evidence command (warning)
//! - `preconditions` every P-NNN line carries a backticked command
//! - `acceptance`    every AC-NNN row has an evidence command and an expected result; each AC
//!   command appears verbatim in verify.toml (warning)
//! - `requirements`  every R-NNN defined in Requirements is cited by an AC row or a checklist
//!   item; a citation on a parent covers every leaf below it; ranges (R-002..R-004) expand
//! - `checklist`     one block; line grammar; IDs contiguous; depth = ID components; max depth 4;
//!   5-20 leaves; every leaf has "evidence:" with non-empty text carrying a backticked command
//!   or an "exit(s) 0" claim (gate leaf exempt); no two items with identical evidence text; no
//!   single-child parent; every AC-* cited on a leaf or on a parent that carries its own
//!   "evidence:"; last leaf is the `taskfmt verify` gate
//! - `commands`      `cargo test` with two or more positional filters and no ' -- ' (warning)
//! - `config`        verify.toml parses; ALLOWED_GLOBS == expected_paths; at least one focused
//!   command; no template placeholders
//! - `size`          README.md over 10,000 bytes (~2,500 tokens) is a warning

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::taskfile::{self, CheckItem, TaskFile};
use crate::verifycfg::{self, FILE_NAME};

pub const SCHEMA: &str = "task/v4";
/// Gate command named by the v4 template frontmatter.
pub const DEFAULT_GATE: &str = "taskfmt verify";
/// Env var naming the template README the `<...>` placeholder set is derived from.
pub const TEMPLATE_ENV: &str = "TASK_TEMPLATE_README";
/// Relative location of the template README inside a checkout.
const TEMPLATE_REL: &str = "reference/task-template/README.md";
/// The template README compiled into the binary: the fallback when no checkout is around (the
/// binary baked into the container image) so the placeholder set is never missing.
const EMBEDDED_TEMPLATE: &str = include_str!("../../reference/task-template/README.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warn,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub severity: Severity,
    pub rule: &'static str,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct LintReport {
    pub target: PathBuf,
    pub findings: Vec<Finding>,
}

impl LintReport {
    pub fn errors(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count()
    }

    pub fn warnings(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Warn)
            .count()
    }

    pub fn passed(&self) -> bool {
        self.errors() == 0
    }

    /// `ERROR …`/`WARN …` lines + `SUMMARY` + `LINT PASS|FAIL`.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for finding in &self.findings {
            match finding.severity {
                Severity::Error => {
                    out.push_str(&format!("ERROR {}: {}\n", finding.rule, finding.message))
                }
                Severity::Warn => {
                    out.push_str(&format!("WARN  {}: {}\n", finding.rule, finding.message))
                }
            }
        }
        out.push_str(&format!(
            "SUMMARY errors={} warnings={}\n",
            self.errors(),
            self.warnings()
        ));
        out.push_str(if self.passed() {
            "LINT PASS\n"
        } else {
            "LINT FAIL\n"
        });
        out
    }
}

/// Lint a task directory or a `README.md` path.
pub fn lint_path(target: &Path) -> LintReport {
    let dir = if target.is_dir() {
        target.to_path_buf()
    } else {
        target.parent().unwrap_or(Path::new(".")).to_path_buf()
    };
    let readme = dir.join("README.md");
    if !readme.is_file() {
        return LintReport {
            target: target.to_path_buf(),
            findings: vec![Finding {
                severity: Severity::Error,
                rule: "readme",
                message: format!("missing {}", readme.display()),
            }],
        };
    }
    match std::fs::read_to_string(&readme) {
        Ok(text) => LintReport {
            target: readme.clone(),
            findings: lint_text(&text, &readme),
        },
        Err(err) => LintReport {
            target: readme,
            findings: vec![Finding {
                severity: Severity::Error,
                rule: "readme",
                message: format!("cannot read: {err}"),
            }],
        },
    }
}

/// The lint itself, on in-memory text (used by selftest, progress-init and the tests).
pub fn lint_text(text: &str, readme: &Path) -> Vec<Finding> {
    let mut findings: Vec<Finding> = Vec::new();

    let dir = readme.parent().unwrap_or(Path::new(".")).to_path_buf();
    let Ok(tf) = TaskFile::parse(text.to_string(), readme) else {
        finding(
            &mut findings,
            "frontmatter",
            "no YAML frontmatter block at top of README.md".to_string(),
        );
        return findings;
    };
    let fm = &tf.frontmatter;

    // ---------- frontmatter ----------
    if fm.schema != SCHEMA {
        let shown = if fm.schema.is_empty() {
            "<missing>".to_string()
        } else {
            fm.schema.clone()
        };
        finding(
            &mut findings,
            "frontmatter",
            format!("schema is '{shown}', want {SCHEMA}"),
        );
    }
    if !Regex::new(r"^TASK-[0-9]+$")
        .expect("static regex")
        .is_match(&fm.id)
    {
        let shown = if fm.id.is_empty() {
            "<missing>".to_string()
        } else {
            fm.id.clone()
        };
        finding(
            &mut findings,
            "frontmatter",
            format!("id is '{shown}', want TASK-<digits>"),
        );
    }
    if fm.title.is_empty() {
        finding(&mut findings, "frontmatter", "title missing".to_string());
    }
    match fm.kind.as_str() {
        "bugfix" | "feature" | "refactor" | "removal" | "migration" | "test" | "docs" => {}
        other => {
            let shown = if other.is_empty() {
                "<missing>".to_string()
            } else {
                other.to_string()
            };
            finding(&mut findings, "frontmatter", format!("kind is '{shown}'"));
        }
    }
    if fm.verify.is_empty() {
        finding(&mut findings, "frontmatter", "verify missing".to_string());
    }
    if fm.expected_paths.is_empty() {
        finding(
            &mut findings,
            "frontmatter",
            "expected_paths empty (scope whitelist)".to_string(),
        );
    }

    // ---------- sections ----------
    let want_sections = [
        "Goal",
        "Context",
        "Preconditions",
        "Scope",
        "Requirements",
        "Acceptance criteria",
        "Fixed decisions",
        "Checklist",
    ];
    let have_sections: Vec<&str> = tf
        .sections
        .iter()
        .map(|(title, _)| title.as_str())
        .collect();
    let missing: Vec<&str> = want_sections
        .iter()
        .copied()
        .filter(|want| !have_sections.contains(want))
        .collect();
    if !missing.is_empty() {
        finding(
            &mut findings,
            "sections",
            format!("missing H2: {}", missing.join(" ")),
        );
    } else {
        let positions: Vec<usize> = want_sections
            .iter()
            .filter_map(|want| have_sections.iter().position(|h| h == want))
            .collect();
        let mut sorted = positions.clone();
        sorted.sort_unstable();
        if positions != sorted {
            finding(
                &mut findings,
                "sections",
                "H2 order differs from template order".to_string(),
            );
        }
    }

    // ---------- heading ----------
    let h1_id = tf.h1.as_deref().and_then(|h1| {
        Regex::new(r"^TASK-[0-9]+")
            .expect("static regex")
            .find(h1)
            .map(|m| m.as_str().to_string())
    });
    if h1_id.as_deref() != Some(fm.id.as_str()) {
        finding(
            &mut findings,
            "heading",
            format!("H1 must start with '# {} — '", fm.id),
        );
    }

    // ---------- placeholders ----------
    placeholder_findings(&mut findings, text);

    // ---------- ids: duplicate definitions per class ----------
    for dup in duplicate_definitions(&tf) {
        finding(
            &mut findings,
            "ids",
            format!("duplicate definition of {dup}"),
        );
    }

    // ---------- context: Read-before-editing hints ----------
    let hints_with_task: Vec<String> = read_before_editing_lines(text)
        .into_iter()
        .filter(|(_, line)| line.contains("/task/"))
        .map(|(n, line)| format!("{n}: {line}"))
        .collect();
    if !hints_with_task.is_empty() {
        finding(
            &mut findings,
            "context",
            format!(
                "\"Read before editing\" (non-normative hints) must not reference /task/ binding docs:\n{}",
                indent(&hints_with_task, "    ")
            ),
        );
    }

    // ---------- preconditions ----------
    let p_id = Regex::new(r"(?m)^- \*\*(P-[0-9]+):\*\*").expect("static regex");
    let p_command = Regex::new(r"— *`[^`]+`").expect("static regex");
    for line in &tf.preconditions {
        let Some(caps) = p_id.captures(line) else {
            continue;
        };
        let pid = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
        if !p_command.is_match(line) {
            finding(
                &mut findings,
                "preconditions",
                format!("{pid} has no backticked command"),
            );
        }
    }
    if !p_id.is_match(text) {
        finding(
            &mut findings,
            "preconditions",
            "no P-NNN entries".to_string(),
        );
    }

    // ---------- acceptance table ----------
    if tf.ac_rows.is_empty() {
        finding(&mut findings, "acceptance", "no AC-NNN rows".to_string());
    }
    {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut reported: BTreeSet<&str> = BTreeSet::new();
        for row in &tf.ac_rows {
            if !seen.insert(row.id.as_str()) && reported.insert(row.id.as_str()) {
                finding(&mut findings, "ids", format!("duplicate AC row {}", row.id));
            }
        }
    }
    let backtick_cmd = Regex::new(r"`[^`]+`").expect("static regex");
    // (AC id, evidence command) for every row that names one
    let mut ac_cmds: Vec<(String, String)> = Vec::new();
    for row in &tf.ac_rows {
        match taskfile::first_code_span(&row.evidence) {
            Some(cmd) if backtick_cmd.is_match(&row.evidence) => {
                ac_cmds.push((row.id.clone(), cmd.to_string()));
            }
            _ => finding(
                &mut findings,
                "acceptance",
                format!("{} evidence column has no backticked command", row.id),
            ),
        }
        if row.expected.chars().all(char::is_whitespace) {
            finding(
                &mut findings,
                "acceptance",
                format!("{} expected column empty", row.id),
            );
        }
        if row.gwt.chars().all(char::is_whitespace) {
            finding(
                &mut findings,
                "acceptance",
                format!("{} Given/When/Then empty", row.id),
            );
        }
    }

    // ---------- checklist ----------
    let n_start = text.matches(taskfile::CHECKLIST_START).count();
    let n_end = text.matches(taskfile::CHECKLIST_END).count();
    if n_start != 1 {
        finding(
            &mut findings,
            "checklist",
            format!("expected exactly one <!-- checklist:start --> marker, found {n_start}"),
        );
    }
    if n_end != 1 {
        finding(
            &mut findings,
            "checklist",
            "expected exactly one <!-- checklist:end --> marker".to_string(),
        );
    }
    let mut items: Vec<CheckItem> = Vec::new();
    if n_start == 1 && n_end == 1 {
        if tf.checklist.is_empty() {
            finding(
                &mut findings,
                "checklist",
                "checklist block empty".to_string(),
            );
        } else {
            items = checklist_findings(&mut findings, &tf);
        }
    }
    let leaves = taskfile::leaf_flags(&items);

    // ---------- requirements: every R-NNN cited by an AC row or a checklist item ----------
    requirement_findings(&mut findings, &tf, &items, &leaves);

    // ---------- baseline: fenced command equals an AC evidence command ----------
    match baseline_command(text) {
        None => findings.push(Finding {
            severity: Severity::Warn,
            rule: "baseline",
            message: "no fenced command found after \"Baseline\"".to_string(),
        }),
        Some(cmd) => {
            if !ac_cmds.iter().any(|(_, ac)| *ac == cmd) {
                findings.push(Finding {
                    severity: Severity::Warn,
                    rule: "baseline",
                    message: format!(
                        "Baseline command is not identical to any AC evidence command: {cmd}"
                    ),
                });
            }
        }
    }

    // ---------- commands: cargo test with several positional filters and no ' -- ' ----------
    for (id, cmd) in &ac_cmds {
        if cargo_multi_filter(cmd) {
            findings.push(Finding {
                severity: Severity::Warn,
                rule: "commands",
                message: format!(
                    "{id}: cargo test takes one positional filter; several words without ' -- ' is invalid: {cmd}"
                ),
            });
        }
    }
    for (item, is_leaf) in items.iter().zip(&leaves) {
        if !*is_leaf || !item.well_formed {
            continue;
        }
        let Some(cmd) = taskfile::leaf_evidence(item)
            .as_deref()
            .and_then(taskfile::first_code_span)
            .map(str::to_string)
        else {
            continue;
        };
        if cargo_multi_filter(&cmd) {
            findings.push(Finding {
                severity: Severity::Warn,
                rule: "commands",
                message: format!(
                    "leaf {}: cargo test takes one positional filter; several words without ' -- ' is invalid: {cmd}",
                    item.id
                ),
            });
        }
    }

    // ---------- verify.toml ----------
    verify_config_findings(&mut findings, &dir, fm, &ac_cmds);

    // ---------- size ----------
    let bytes = text.len();
    if bytes > 10_000 {
        findings.push(Finding {
            severity: Severity::Warn,
            rule: "size",
            message: format!(
                "README.md is {bytes} bytes (~{} tokens); target under ~2,500 tokens",
                bytes / 4
            ),
        });
    }

    findings
}

fn indent(lines: &[String], prefix: &str) -> String {
    lines
        .iter()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn finding(findings: &mut Vec<Finding>, rule: &'static str, message: String) {
    findings.push(Finding {
        severity: Severity::Error,
        rule,
        message,
    });
}

// ---------------------------------------------------------------------------------------------
// placeholders
// ---------------------------------------------------------------------------------------------

/// The template README text and where it came from: `TASK_TEMPLATE_README` (must exist), else
/// `reference/task-template/README.md` under the nearest ancestor of the cwd or the compile-time
/// repo root, else the copy embedded at build time.
fn template_readme() -> Result<(String, String), String> {
    if let Some(path) = std::env::var_os(TEMPLATE_ENV).filter(|v| !v.is_empty()) {
        let path = PathBuf::from(path);
        return std::fs::read_to_string(&path)
            .map(|text| (text, path.display().to_string()))
            .map_err(|_| {
                format!(
                    "template README not found at {} (set {TEMPLATE_ENV}); cannot derive the placeholder set",
                    path.display()
                )
            });
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.extend(cwd.ancestors().map(|dir| dir.join(TEMPLATE_REL)));
    }
    candidates.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(TEMPLATE_REL),
    );
    for candidate in candidates {
        if let Ok(text) = std::fs::read_to_string(&candidate) {
            return Ok((text, candidate.display().to_string()));
        }
    }
    Ok((
        EMBEDDED_TEMPLATE.to_string(),
        format!("<embedded {TEMPLATE_REL}>"),
    ))
}

/// Every `<...>` span of the template (HTML comments excluded), plus each span with its inline
/// code removed — the shape a README line takes once its code spans are dropped.
pub fn template_placeholder_set(template: &str) -> BTreeSet<String> {
    let span = Regex::new(r"<[^<>]+>").expect("static regex");
    let code = Regex::new(r"`[^`]*`").expect("static regex");
    let mut set = BTreeSet::new();
    for line in template.lines() {
        for m in span.find_iter(line) {
            let p = m.as_str();
            if p.starts_with("<!--") {
                continue;
            }
            set.insert(p.to_string());
            set.insert(code.replace_all(p, "").into_owned());
        }
    }
    set
}

/// `<...>` spans of one README line in placeholder position: a span that is the whole content of
/// an inline-code span is unwrapped; every other code span is dropped (its `<...>` is literal).
pub fn bare_spans(line: &str) -> Vec<String> {
    let whole = Regex::new(r"`(<[^<>`]+>)`").expect("static regex");
    let code = Regex::new(r"`[^`]*`").expect("static regex");
    let span = Regex::new(r"<[^<>]+>").expect("static regex");
    let unwrapped = whole.replace_all(line, "$1");
    let stripped = code.replace_all(&unwrapped, "");
    span.find_iter(&stripped)
        .map(|m| m.as_str().to_string())
        .filter(|p| !p.starts_with("<!--"))
        .collect()
}

fn placeholder_findings(findings: &mut Vec<Finding>, text: &str) {
    let hand_listed = Regex::new(r"TASK-000|\b[PRD]-NNN\b|AC-NNN").expect("static regex");
    let hits: Vec<String> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| hand_listed.is_match(line))
        .map(|(i, line)| format!("{}:{}", i + 1, line))
        .collect();
    if !hits.is_empty() {
        finding(
            findings,
            "placeholders",
            format!("template placeholders left:\n{}", indent(&hits, "    ")),
        );
    }
    let (template, source) = match template_readme() {
        Ok(found) => found,
        Err(message) => {
            finding(findings, "placeholders", message);
            return;
        }
    };
    let set = template_placeholder_set(&template);
    let mut errors: Vec<String> = Vec::new();
    let mut warns: Vec<String> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        for p in bare_spans(line) {
            if set.contains(&p) {
                errors.push(format!("{}: {p}", i + 1));
            } else if p.contains(' ') {
                warns.push(format!("{}: {p}", i + 1));
            }
        }
    }
    if !errors.is_empty() {
        finding(
            findings,
            "placeholders",
            format!(
                "template <...> placeholders left (set derived from {source}):\n{}",
                indent(&errors, "    ")
            ),
        );
    }
    if !warns.is_empty() {
        findings.push(Finding {
            severity: Severity::Warn,
            rule: "placeholders",
            message: format!(
                "bare <...> spans outside inline code look unfilled:\n{}",
                indent(&warns, "    ")
            ),
        });
    }
}

// ---------------------------------------------------------------------------------------------
// ids, context, baseline, requirements
// ---------------------------------------------------------------------------------------------

/// Duplicate `P-`/`R-`/`D-` definitions (`- **X-NNN …**` lines in the three defining sections),
/// in document order, each reported once per repeat.
fn duplicate_definitions(tf: &TaskFile) -> Vec<String> {
    let id_re = Regex::new(r"[PRD]-[0-9]+").expect("static regex");
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut dups = Vec::new();
    for (title, body) in &tf.sections {
        if !matches!(
            title.as_str(),
            "Preconditions" | "Requirements" | "Fixed decisions"
        ) {
            continue;
        }
        for line in body {
            let Some(token) = definition_token(line) else {
                continue;
            };
            for m in id_re.find_iter(token) {
                if !seen.insert(m.as_str().to_string()) {
                    dups.push(m.as_str().to_string());
                }
            }
        }
    }
    dups
}

/// The bold ID token of a `- **R-001 (MUST):**` line: the text between `- **` and the next `**`.
fn definition_token(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("- **")?;
    if !Regex::new(r"^[PRD]-[0-9]+")
        .expect("static regex")
        .is_match(rest)
    {
        return None;
    }
    let end = rest.find("**")?;
    Some(&rest[..end])
}

/// Expand every `CLASS-NNN` and `CLASS-NNN..CLASS-MMM` citation in `text` (a preceding letter
/// disqualifies: `PR-001` is not `R-001`).
pub fn expand_ids(class: char, text: &str) -> Vec<String> {
    let re = Regex::new(&format!(
        r"[^A-Za-z]{class}-([0-9]+)(?:\.\.{class}-([0-9]+))?"
    ))
    .expect("static regex");
    let mut out = Vec::new();
    for line in text.lines() {
        let padded = format!(" {line}");
        for caps in re.captures_iter(&padded) {
            let lo_raw = caps.get(1).map(|m| m.as_str()).unwrap_or("0");
            let Ok(lo) = lo_raw.parse::<u32>() else {
                continue;
            };
            match caps.get(2).and_then(|m| m.as_str().parse::<u32>().ok()) {
                Some(hi) => {
                    let width = lo_raw.len();
                    for n in lo..=hi {
                        out.push(format!("{class}-{n:0width$}"));
                    }
                }
                None => out.push(format!("{class}-{lo_raw}")),
            }
        }
    }
    out
}

/// `(line number, line)` of every numbered entry in the "Read before editing" list.
fn read_before_editing_lines(text: &str) -> Vec<(usize, String)> {
    let numbered = Regex::new(r"^[0-9]+\. ").expect("static regex");
    let mut out = Vec::new();
    let mut in_list = false;
    let mut started = false;
    for (i, line) in text.lines().enumerate() {
        if !in_list {
            if line.starts_with("Read before editing") {
                in_list = true;
            }
            continue;
        }
        if numbered.is_match(line) {
            started = true;
            out.push((i + 1, line.to_string()));
            continue;
        }
        if started && !line.starts_with(' ') {
            break;
        }
    }
    out
}

/// First non-blank line of the first fenced block after the `Baseline` paragraph.
fn baseline_command(text: &str) -> Option<String> {
    let mut after_baseline = false;
    let mut in_fence = false;
    for line in text.lines() {
        if !after_baseline {
            after_baseline = line.starts_with("Baseline");
            continue;
        }
        if line.starts_with("```") {
            if in_fence {
                return None;
            }
            in_fence = true;
            continue;
        }
        if in_fence && !line.trim().is_empty() {
            return Some(line.trim().to_string());
        }
    }
    None
}

fn requirement_findings(
    findings: &mut Vec<Finding>,
    tf: &TaskFile,
    items: &[CheckItem],
    leaves: &[bool],
) {
    let defined: BTreeSet<String> = tf
        .section("Requirements")
        .map(|body| {
            body.iter()
                .filter_map(|line| definition_token(line))
                .filter(|token| token.starts_with("R-"))
                .flat_map(|token| expand_ids('R', token))
                .collect()
        })
        .unwrap_or_default();
    if defined.is_empty() {
        finding(findings, "requirements", "no R-NNN entries".to_string());
        return;
    }
    let mut cited_text = String::new();
    for row in &tf.ac_rows {
        cited_text.push_str(&format!("{} {} {}\n", row.gwt, row.evidence, row.expected));
    }
    // a leaf plus its ancestors: a citation on a parent covers every leaf below it
    let mut ancestors: Vec<String> = Vec::new();
    for (item, is_leaf) in items.iter().zip(leaves) {
        if !item.well_formed {
            continue;
        }
        ancestors.truncate(item.depth);
        ancestors.push(item.raw.clone());
        if *is_leaf {
            cited_text.push_str(&ancestors.join(" || "));
            cited_text.push('\n');
        }
    }
    let cited: BTreeSet<String> = expand_ids('R', &cited_text).into_iter().collect();
    for r in defined.difference(&cited) {
        finding(
            findings,
            "requirements",
            format!("{r} is not cited by any AC row or checklist leaf"),
        );
    }
}

// ---------------------------------------------------------------------------------------------
// commands
// ---------------------------------------------------------------------------------------------

/// Flags of `cargo test` that consume the next token.
const CARGO_VALUE_FLAGS: &[&str] = &[
    "-p",
    "--package",
    "--test",
    "--bin",
    "--example",
    "--bench",
    "--features",
    "-F",
    "--target",
    "--manifest-path",
    "-j",
    "--jobs",
    "--profile",
    "--exclude",
    "--target-dir",
    "--color",
    "--message-format",
    "--config",
    "-Z",
];

/// `cargo test` with two or more positional filter words and no ` -- ` separator: cargo takes one
/// filter; the rest is an error.
pub fn cargo_multi_filter(command: &str) -> bool {
    if command.contains(" -- ") || command.ends_with(" --") {
        return false;
    }
    let mut state = 0u8;
    let mut positional = 0usize;
    let mut skip = false;
    for tok in command.split_whitespace() {
        match state {
            0 => {
                if tok == "cargo" {
                    state = 1;
                }
                continue;
            }
            1 => {
                if tok.starts_with('+') {
                    continue;
                }
                if tok != "test" {
                    return false;
                }
                state = 2;
                continue;
            }
            _ => {}
        }
        if skip {
            skip = false;
            continue;
        }
        if CARGO_VALUE_FLAGS.contains(&tok) {
            skip = true;
        } else if tok.starts_with('-') {
        } else if matches!(tok, "|" | "||" | "&&" | ";" | ">" | "2>") {
            break;
        } else {
            positional += 1;
        }
    }
    positional >= 2
}

// ---------------------------------------------------------------------------------------------
// checklist
// ---------------------------------------------------------------------------------------------

/// Checklist grammar, contiguity, depth, leaf and coverage rules (the awk block, ported line for
/// line so finding order matches the shell original). Returns the parsed items for later rules.
fn checklist_findings(findings: &mut Vec<Finding>, tf: &TaskFile) -> Vec<CheckItem> {
    let items = taskfile::parse_checklist(&tf.checklist);
    let mut seen: Vec<&str> = Vec::new();

    for (index, item) in items.iter().enumerate() {
        if !item.well_formed {
            finding(findings, "checklist", format!("bad line: {}", item.raw));
            continue;
        }
        if item.id.split('.').count() != item.depth + 1 {
            finding(
                findings,
                "checklist",
                format!("depth/ID mismatch: {}", item.id),
            );
        }
        if item.depth > 3 {
            finding(findings, "checklist", format!("depth > 4: {}", item.id));
        }
        if seen.contains(&item.id.as_str()) {
            finding(findings, "checklist", format!("duplicate ID {}", item.id));
        }
        seen.push(item.id.as_str());

        if index == 0 {
            if item.id != "1" {
                finding(
                    findings,
                    "checklist",
                    format!("first ID must be 1, got {}", item.id),
                );
            }
        } else {
            let prev = &items[index - 1];
            match expected_id(prev, item.depth) {
                Ok(want) => {
                    if item.id != want {
                        finding(
                            findings,
                            "checklist",
                            format!("expected ID {want} after {}, got {}", prev.id, item.id),
                        );
                    }
                }
                Err(()) => finding(
                    findings,
                    "checklist",
                    format!("depth jumps by more than one at {}", item.id),
                ),
            }
        }
    }

    // structural findings
    let leaves = taskfile::leaf_flags(&items);
    let mut leaf_count = 0usize;
    for (i, item) in items.iter().enumerate() {
        if leaves[i] {
            leaf_count += 1;
            if !item.text.contains("evidence:") {
                finding(
                    findings,
                    "checklist",
                    format!("leaf {} has no \"evidence:\"", item.id),
                );
            }
        } else {
            let kids = items[i + 1..]
                .iter()
                .take_while(|candidate| candidate.depth > item.depth)
                .filter(|candidate| candidate.depth == item.depth + 1)
                .count();
            if kids == 1 {
                finding(
                    findings,
                    "checklist",
                    format!("parent {} has a single child", item.id),
                );
            }
        }
    }
    if !(5..=20).contains(&leaf_count) {
        finding(
            findings,
            "checklist",
            format!("{leaf_count} leaves, want 5-20"),
        );
    }
    if let Some(last) = items.last() {
        let gate = if tf.frontmatter.verify.is_empty() {
            DEFAULT_GATE.to_string()
        } else {
            tf.frontmatter.verify.clone()
        };
        // v4 names the gate command; the old bash gate (verify.sh) is gone.
        if !last.text.contains(&gate) {
            finding(
                findings,
                "checklist",
                format!("last leaf {} must be the `{gate}` gate leaf", last.id),
            );
        }
    }

    // leaf evidence text, duplicates; AC coverage (leaf or evidence-bearing parent)
    let gate_leaf = items
        .iter()
        .zip(&leaves)
        .rfind(|(_, is_leaf)| **is_leaf)
        .map(|(item, _)| item.id.clone());
    let backtick_cmd = Regex::new(r"`[^`]+`").expect("static regex");
    let exit_claim = Regex::new(r"exits? 0").expect("static regex");
    let mut first_with: HashMap<String, (String, bool)> = HashMap::new();
    let mut ac_text = String::new();
    for (item, is_leaf) in items.iter().zip(&leaves) {
        if !item.well_formed {
            continue;
        }
        let Some(ev) = taskfile::leaf_evidence(item) else {
            continue; // reported by the grammar pass above
        };
        if *is_leaf {
            if ev.is_empty() {
                finding(
                    findings,
                    "checklist",
                    format!("leaf {} evidence text is empty", item.id),
                );
            } else if gate_leaf.as_deref() != Some(item.id.as_str())
                && !backtick_cmd.is_match(&ev)
                && !exit_claim.is_match(&ev)
            {
                finding(
                    findings,
                    "checklist",
                    format!(
                        "leaf {} evidence names no backticked command and no 'exit 0' claim: {ev}",
                        item.id
                    ),
                );
            }
        }
        ac_text.push_str(&item.raw);
        ac_text.push('\n');
        match first_with.get(&ev) {
            Some((first_id, first_leaf)) => {
                let noun = if *first_leaf && *is_leaf {
                    "leaves"
                } else {
                    "items"
                };
                finding(
                    findings,
                    "checklist",
                    format!(
                        "{noun} {first_id} and {} carry identical evidence: {ev}",
                        item.id
                    ),
                );
            }
            None => {
                first_with.insert(ev, (item.id.clone(), *is_leaf));
            }
        }
    }
    for ac in &tf.ac_rows {
        if !ac_text.contains(&format!("`{}`", ac.id)) {
            finding(
                findings,
                "checklist",
                format!(
                    "{} is not cited by any leaf or evidence-bearing parent",
                    ac.id
                ),
            );
        }
    }
    items
}

/// The ID the checklist grammar expects after `prev` at `depth`. `Err(())` = the depth jumped by
/// more than one level.
fn expected_id(prev: &CheckItem, depth: usize) -> Result<String, ()> {
    if depth == prev.depth + 1 {
        return Ok(format!("{}.1", prev.id));
    }
    if depth > prev.depth {
        return Err(());
    }
    let parts: Vec<u32> = prev
        .id
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| ())?;
    if parts.len() < depth + 1 {
        return Err(());
    }
    let mut components: Vec<String> = parts.iter().take(depth + 1).map(u32::to_string).collect();
    components[depth] = (parts[depth] + 1).to_string();
    Ok(components.join("."))
}

// ---------------------------------------------------------------------------------------------
// verify.toml
// ---------------------------------------------------------------------------------------------

/// `verify.toml` ↔ frontmatter consistency (the sourced `verify.config` checks, ported to toml),
/// plus the "every AC command is run by the gate" warning.
fn verify_config_findings(
    findings: &mut Vec<Finding>,
    dir: &Path,
    fm: &taskfile::Frontmatter,
    ac_cmds: &[(String, String)],
) {
    let path = dir.join(FILE_NAME);
    if !path.is_file() {
        findings.push(Finding {
            severity: Severity::Warn,
            rule: "config",
            message: format!("no {FILE_NAME} next to README.md"),
        });
        return;
    }
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            finding(
                findings,
                "config",
                format!("{FILE_NAME} cannot be read: {err}"),
            );
            return;
        }
    };
    let cfg = match verifycfg::VerifyConfig::parse(&text) {
        Ok(cfg) => cfg,
        Err(err) => {
            let detail: Vec<String> = format!("{err:#}").lines().map(str::to_string).collect();
            finding(
                findings,
                "config",
                format!("{FILE_NAME} does not parse:\n{}", indent(&detail, "    ")),
            );
            return;
        }
    };

    if cfg.base_ref.as_deref() == Some("") {
        finding(findings, "config", "BASE_REF empty".to_string());
    }
    if cfg.focused.commands.is_empty() {
        finding(findings, "config", "FOCUSED_CMDS empty".to_string());
    }

    let mut allowed = cfg.allowed_globs.clone();
    allowed.sort();
    allowed.dedup();
    let mut expected = fm.expected_paths.clone();
    expected.sort();
    expected.dedup();
    if allowed != expected {
        let mut diff: Vec<String> = Vec::new();
        diff.extend(
            expected
                .iter()
                .filter(|item| !allowed.contains(item))
                .map(|item| format!("- {item}")),
        );
        diff.extend(
            allowed
                .iter()
                .filter(|item| !expected.contains(item))
                .map(|item| format!("+ {item}")),
        );
        finding(
            findings,
            "config",
            format!(
                "ALLOWED_GLOBS != expected_paths:\n{}",
                indent(&diff, "    ")
            ),
        );
    }

    let code: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let placeholder = Regex::new(r#"<[^<>"]+>"#).expect("static regex");
    let mut found: Vec<String> = placeholder
        .find_iter(&code)
        .map(|m| m.as_str().to_string())
        .collect();
    found.sort();
    found.dedup();
    if !found.is_empty() {
        finding(
            findings,
            "config",
            format!(
                "template placeholders left in verify.toml: {}",
                found.join(" ")
            ),
        );
    }
    // every AC evidence command should be run by the gate: verbatim substring of the config text
    for (id, cmd) in ac_cmds {
        if !code.contains(cmd.as_str()) {
            findings.push(Finding {
                severity: Severity::Warn,
                rule: "acceptance",
                message: format!(
                    "{id} evidence command does not appear verbatim in {FILE_NAME}: {cmd}"
                ),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_warning_above_ten_thousand_bytes() {
        let mut text = String::from(
            "---\nschema: task/v4\nid: TASK-001\ntitle: t\nkind: bugfix\nverify: \"taskfmt verify\"\nexpected_paths:\n  - \"src/*\"\n---\n",
        );
        text.push_str("\n## Goal\n\n");
        text.push_str(&"x".repeat(10_500));
        let findings = lint_text(&text, Path::new("/tmp/README.md"));
        assert!(
            findings
                .iter()
                .any(|f| f.rule == "size" && f.severity == Severity::Warn)
        );
    }

    #[test]
    fn contiguity_wants_next_sibling_incremented() {
        let prev = CheckItem {
            raw: "    - [ ] **2.3** x".into(),
            indent: 4,
            depth: 1,
            id: "2.3".into(),
            text: "x".into(),
            checked: false,
            well_formed: true,
        };
        assert_eq!(expected_id(&prev, 1).unwrap(), "2.4"); // next sibling
        assert_eq!(expected_id(&prev, 2).unwrap(), "2.3.1"); // first child
        assert_eq!(expected_id(&prev, 0).unwrap(), "3"); // sibling of the parent
        assert!(expected_id(&prev, 3).is_err()); // deeper than a child
    }

    #[test]
    fn placeholder_set_is_derived_from_the_embedded_template() {
        let set = template_placeholder_set(EMBEDDED_TEMPLATE);
        assert!(set.contains("<command>"));
        assert!(set.contains("<area>"));
        // the code-stripped twin of `<Coherent unit satisfying `R-001`>`
        assert!(set.contains("<Coherent unit satisfying >"));
        assert!(!set.iter().any(|p| p.starts_with("<!--")));
    }

    #[test]
    fn bare_spans_unwrap_whole_code_spans_and_drop_literal_ones() {
        assert_eq!(bare_spans("run `<command>` now"), vec!["<command>"]);
        assert!(bare_spans("run `pgtui --db <path>` and `Vec<TableRef>`").is_empty());
        assert_eq!(bare_spans("Given <state>, when x"), vec!["<state>"]);
        assert!(bare_spans("<!-- checklist:start -->").is_empty());
    }

    #[test]
    fn expand_ids_handles_ranges_and_letter_prefixes() {
        assert_eq!(
            expand_ids('R', "R-002..R-004 and PR-009 and `R-001`"),
            vec!["R-002", "R-003", "R-004", "R-001"]
        );
    }

    #[test]
    fn cargo_multi_filter_detects_two_positionals() {
        assert!(cargo_multi_filter("cargo test -p auth expired valid"));
        assert!(!cargo_multi_filter("cargo test -p auth expired"));
        assert!(!cargo_multi_filter("cargo test expired -- valid"));
        assert!(!cargo_multi_filter("cargo test --test suite expired"));
        assert!(!cargo_multi_filter("cargo build a b"));
        assert!(cargo_multi_filter("cargo +nightly test a b | tee log"));
        assert!(!cargo_multi_filter("cargo test a | grep b"));
    }

    #[test]
    fn baseline_command_reads_the_first_fenced_line() {
        let text = "## Context\n\nBaseline (run):\n\n```sh\ncargo test x\n```\n";
        assert_eq!(baseline_command(text).as_deref(), Some("cargo test x"));
        assert_eq!(baseline_command("Baseline:\n\nno fence\n"), None);
    }
}
