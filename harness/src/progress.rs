//! progress.md: generation (`progress-init`) and parsing.
//!
//! `progress.md` is derived state — generated from `README.md` at dispatch, never committed. The
//! header sits between `---` fences so a markdown viewer renders it; the checklist block is a
//! verbatim copy of the README one (only the `[ ]`/`[x]` tokens may ever differ).

use std::path::Path;

use anyhow::{Context, bail};
use regex::Regex;

use crate::lint;
use crate::taskfile::{self, TaskFile};

#[derive(Debug, Clone)]
pub struct GeneratedProgress {
    pub task: String,
    pub first_leaf: String,
    pub body: String,
}

/// Generate the initial `progress.md` body for a task package. Lints first: an invalid contract
/// never yields a progress file.
pub fn generate(task_dir: &Path) -> anyhow::Result<GeneratedProgress> {
    let readme = task_dir.join("README.md");
    if !readme.is_file() {
        bail!("progress-init: missing {}", readme.display());
    }
    let text = std::fs::read_to_string(&readme)
        .with_context(|| format!("cannot read {}", readme.display()))?;
    let findings = lint::lint_text(&text, &readme);
    let errors = findings
        .iter()
        .filter(|f| f.severity == lint::Severity::Error)
        .count();
    if errors > 0 {
        let report = lint::LintReport {
            target: readme,
            findings,
        };
        bail!(
            "progress-init: README.md failed task-lint ({} error(s)); not generating\n{}",
            errors,
            report.render()
        );
    }

    let tf = TaskFile::parse(text, &readme)?;
    let id = tf.frontmatter.id.clone();
    if id.is_empty() {
        bail!("progress-init: README.md has no id in its frontmatter");
    }
    let items = taskfile::parse_checklist(&tf.checklist);
    let first_leaf = taskfile::first_leaf(&items).ok_or_else(|| {
        anyhow::anyhow!(
            "progress-init: no checklist leaf found in {}",
            readme.display()
        )
    })?;

    let mut body = String::new();
    body.push_str("---\n");
    body.push_str(&format!("TASK: {id}\n"));
    body.push_str("STATE: IN_PROGRESS\n");
    body.push_str(&format!("CURRENT: {first_leaf}\n"));
    body.push_str("BASELINE: <not run>\n");
    body.push_str("---\n\n");
    body.push_str(&format!("{}\n", taskfile::CHECKLIST_START));
    for line in &tf.checklist {
        body.push_str(line);
        body.push('\n');
    }
    body.push_str(&format!("{}\n\n", taskfile::CHECKLIST_END));
    body.push_str("## Log\n\n");
    body.push_str("## Handoff\n");
    body.push_str(&format!("NEXT: {first_leaf}\n"));
    body.push_str("CURRENT_FAILURE: none\n");
    body.push_str("DECISIONS: none\n");

    Ok(GeneratedProgress {
        task: id,
        first_leaf,
        body,
    })
}

/// Parse the parts of a progress file the gate and the status machinery need.
#[derive(Debug, Clone, Default)]
pub struct ProgressFile {
    pub task: String,
    pub state: String,
    pub current: String,
    pub baseline: String,
    pub checklist: Vec<String>,
    pub log_lines: Vec<String>,
}

impl ProgressFile {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read progress file {}", path.display()))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let mut out = ProgressFile::default();
        let mut in_checklist = false;
        for line in text.lines() {
            let trimmed = line.trim_end();
            if trimmed == taskfile::CHECKLIST_START {
                in_checklist = true;
                continue;
            }
            if trimmed == taskfile::CHECKLIST_END {
                in_checklist = false;
                continue;
            }
            if in_checklist {
                out.checklist.push(line.to_string());
                continue;
            }
            let content = line.trim();
            if let Some(rest) = content.strip_prefix("TASK:") {
                out.task = rest.trim().to_string();
            } else if let Some(rest) = content.strip_prefix("STATE:") {
                out.state = rest.trim().to_string();
            } else if let Some(rest) = content.strip_prefix("CURRENT:") {
                out.current = rest.trim().to_string();
            } else if let Some(rest) = content.strip_prefix("BASELINE:") {
                out.baseline = rest.trim().to_string();
            } else if let Some(rest) = content.strip_prefix("- ") {
                out.log_lines.push(rest.to_string());
            }
        }
        Ok(out)
    }
}

/// Header value of a progress file (`TASK`, `STATE`, `CURRENT`, `BASELINE`).
pub fn header_value(text: &str, key: &str) -> Option<String> {
    let re = Regex::new(&format!(r"^{key} *:? *(.*)$")).ok()?;
    text.lines().find_map(|line| re.captures(line)).map(|caps| {
        caps.get(1)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_values_are_extracted() {
        let text = "STATE: IN_PROGRESS\nCURRENT: 1.1\n";
        assert_eq!(header_value(text, "STATE").as_deref(), Some("IN_PROGRESS"));
        assert_eq!(header_value(text, "CURRENT").as_deref(), Some("1.1"));
        assert_eq!(header_value(text, "BASELINE"), None);
    }

    #[test]
    fn parses_generated_file() {
        let body = "---\nTASK: TASK-042\nSTATE: DONE\nCURRENT: NONE\nBASELINE: cargo test -> 1 failed\n---\n\n<!-- checklist:start -->\n- [x] **1** x\n<!-- checklist:end -->\n\n## Log\n- 1 | DONE | cargo test -> ok\n\n## Handoff\n";
        let parsed = ProgressFile::parse(body).unwrap();
        assert_eq!(parsed.task, "TASK-042");
        assert_eq!(parsed.state, "DONE");
        assert_eq!(parsed.current, "NONE");
        assert_eq!(parsed.baseline, "cargo test -> 1 failed");
        assert_eq!(parsed.checklist, vec!["- [x] **1** x".to_string()]);
        assert_eq!(
            parsed.log_lines,
            vec!["1 | DONE | cargo test -> ok".to_string()]
        );
    }
}
