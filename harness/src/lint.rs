//! Task lint — a faithful port of `harness/task-lint.sh` (schema `task/v4`).
//!
//! Exit 0 = every rule passed. Exit 1 = at least one ERROR. Warnings never fail.
//! Output: one line per finding — `ERROR <rule>: <detail>` or `WARN  <rule>: <detail>` — then
//! `SUMMARY errors=N warnings=M` and `LINT PASS|FAIL`. Messages are byte-identical to the shell
//! original so existing fixtures and expectations keep working.

use std::path::{Path, PathBuf};

use regex::Regex;

use crate::taskfile::{self, CheckItem, TaskFile};
use crate::verifycfg::{self, FILE_NAME};

pub const SCHEMA: &str = "task/v4";
/// Gate command named by the v4 template frontmatter.
pub const DEFAULT_GATE: &str = "taskfmt verify";

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
    let placeholders = Regex::new(
        r"TASK-000|\b[PRD]-NNN\b|AC-NNN|<command>|<expected>|<result>|<state>|<imperative|<One sentence|<area>",
    )
    .expect("static regex");
    let hits: Vec<String> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| placeholders.is_match(line))
        .map(|(i, line)| format!("{}:{}", i + 1, line))
        .collect();
    if !hits.is_empty() {
        finding(
            &mut findings,
            "placeholders",
            format!("template placeholders left:\n{}", indent(&hits, "    ")),
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
    let backtick_cmd = Regex::new(r"`[^`]+`").expect("static regex");
    for row in &tf.ac_rows {
        if !backtick_cmd.is_match(&row.evidence) {
            finding(
                &mut findings,
                "acceptance",
                format!("{} evidence column has no backticked command", row.id),
            );
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
    if n_start == 1 && n_end == 1 {
        if tf.checklist.is_empty() {
            finding(
                &mut findings,
                "checklist",
                "checklist block empty".to_string(),
            );
        } else {
            checklist_findings(&mut findings, &tf);
        }
    }

    // ---------- verify.toml ----------
    verify_config_findings(&mut findings, &dir, fm);

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

/// Checklist grammar, contiguity, depth, leaf and coverage rules (the awk block, ported line for
/// line so finding order matches the shell original).
fn checklist_findings(findings: &mut Vec<Finding>, tf: &TaskFile) {
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
    let all_text: String = items
        .iter()
        .map(|item| item.raw.clone())
        .collect::<Vec<_>>()
        .join("\n");
    for ac in &tf.ac_rows {
        if !all_text.contains(&format!("`{}`", ac.id)) {
            finding(
                findings,
                "checklist",
                format!("{} not referenced by any checklist item", ac.id),
            );
        }
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

/// `verify.toml` ↔ frontmatter consistency (the sourced `verify.config` checks, ported to toml).
fn verify_config_findings(findings: &mut Vec<Finding>, dir: &Path, fm: &taskfile::Frontmatter) {
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

    let placeholder = Regex::new(r"<[a-z_ -]+>").expect("static regex");
    let mut found: Vec<String> = text
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .flat_map(|line| {
            placeholder
                .find_iter(line)
                .map(|m| m.as_str().to_string())
                .collect::<Vec<_>>()
        })
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
}
