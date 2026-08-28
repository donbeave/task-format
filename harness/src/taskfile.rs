//! `README.md` task-file parser: YAML-ish frontmatter, H2 sections, the checklist block,
//! precondition lines and acceptance-table rows. Hand-parsed on purpose — the frontmatter is a
//! small fixed schema and the checklist grammar is bespoke.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use regex::Regex;

pub const CHECKLIST_START: &str = "<!-- checklist:start -->";
pub const CHECKLIST_END: &str = "<!-- checklist:end -->";

/// Frontmatter keys the task contract uses.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frontmatter {
    pub schema: String,
    pub id: String,
    pub title: String,
    pub kind: String,
    pub verify: String,
    pub expected_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AcRow {
    pub id: String,
    pub gwt: String,
    pub evidence: String,
    pub expected: String,
}

#[derive(Debug, Clone)]
pub struct TaskFile {
    pub path: PathBuf,
    pub text: String,
    pub frontmatter: Frontmatter,
    /// Every `## Section` heading in document order with its body lines.
    pub sections: Vec<(String, Vec<String>)>,
    /// Raw lines between the checklist markers.
    pub checklist: Vec<String>,
    /// Acceptance rows in document order.
    pub ac_rows: Vec<AcRow>,
    /// Body lines of the `Preconditions` section.
    pub preconditions: Vec<String>,
    /// The first `# H1` line, if any.
    pub h1: Option<String>,
}

impl TaskFile {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        Self::parse(text, path)
    }

    pub fn parse(text: String, path: &Path) -> anyhow::Result<Self> {
        let raw_fm = frontmatter_block(&text).ok_or_else(|| {
            anyhow::anyhow!("no YAML frontmatter block at top of {}", path.display())
        })?;
        let frontmatter = Frontmatter {
            schema: fm_val(&raw_fm, "schema").unwrap_or_default(),
            id: fm_val(&raw_fm, "id").unwrap_or_default(),
            title: fm_val(&raw_fm, "title").unwrap_or_default(),
            kind: fm_val(&raw_fm, "kind").unwrap_or_default(),
            verify: fm_val(&raw_fm, "verify").unwrap_or_default(),
            expected_paths: fm_list(&raw_fm, "expected_paths"),
        };
        let sections = parse_sections(&text);
        let checklist = checklist_block(&text);
        let preconditions = section_body(&sections, "Preconditions")
            .map(|body| {
                body.iter()
                    .filter_map(|line| precondition_id(line).map(|id| (id, line.clone())))
                    .map(|(_, line)| line)
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        let ac_rows = acceptance_rows(
            section_body(&sections, "Acceptance criteria")
                .cloned()
                .unwrap_or_default(),
        );
        let h1 = text.lines().find_map(|line| {
            let trimmed = line.trim_end();
            trimmed.strip_prefix("# ").map(str::to_string)
        });
        Ok(Self {
            path: path.to_path_buf(),
            text,
            frontmatter,
            sections,
            checklist,
            ac_rows,
            preconditions,
            h1,
        })
    }

    pub fn section(&self, title: &str) -> Option<&Vec<String>> {
        section_body(&self.sections, title)
    }

    /// Frontmatter `id` of a README without building the whole parse tree.
    pub fn id(&self) -> &str {
        &self.frontmatter.id
    }
}

/// Lines of the frontmatter block, without the `---` fences. `None` when the file does not start
/// with one.
pub fn frontmatter_block(text: &str) -> Option<String> {
    let mut lines = text.lines();
    if lines.next()?.trim_end() != "---" {
        return None;
    }
    let mut out = Vec::new();
    for line in lines {
        if line.trim_end() == "---" {
            return Some(out.join("\n"));
        }
        out.push(line.to_string());
    }
    None
}

/// Scalar frontmatter value. Mirrors the original `sed -nE "s/^$1: *\"?([^\"#]*[^\" #])\"? *(#.*)?$/\1/p"`:
/// a `#` starts a comment only when preceded by whitespace, quotes are optional and stripped.
pub fn fm_val(fm: &str, key: &str) -> Option<String> {
    let re = Regex::new(&format!(r"^{}\s*:\s*(.*)$", regex::escape(key))).ok()?;
    for line in fm.lines() {
        let Some(caps) = re.captures(line.trim_end()) else {
            continue;
        };
        let rest = caps.get(1)?.as_str().trim();
        // strip a trailing comment: " # ..." (a '#' inside an unquoted value is not supported,
        // matching the original pattern which stops the value at '#')
        let value = match rest.find(" #") {
            Some(idx) => rest[..idx].trim(),
            None => rest,
        };
        let value = value.trim_matches('"').trim();
        return if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        };
    }
    None
}

/// Block list values: lines after `key:` shaped `  - "item"` until the next non-indented line.
pub fn fm_list(fm: &str, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_list = false;
    let key_prefix = format!("{key}:");
    for line in fm.lines() {
        if line.starts_with(key_prefix.as_str()) {
            in_list = true;
            continue;
        }
        if !in_list {
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('-') {
            break;
        }
        let trimmed = line.trim_start();
        if let Some(item) = trimmed.strip_prefix("- ") {
            let item = item.split(" #").next().unwrap_or(item);
            out.push(item.trim().trim_matches('"').to_string());
        } else if !trimmed.is_empty() {
            break;
        }
    }
    out.retain(|item| !item.is_empty());
    out
}

/// Every H2 section in document order: `(title, body lines)`.
pub fn parse_sections(text: &str) -> Vec<(String, Vec<String>)> {
    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_end();
        if let Some(rest) = trimmed.strip_prefix("## ") {
            sections.push((rest.trim().to_string(), Vec::new()));
        } else if let Some(last) = sections.last_mut() {
            last.1.push(line.to_string());
        }
    }
    sections
}

pub fn section_body<'a>(
    sections: &'a [(String, Vec<String>)],
    title: &str,
) -> Option<&'a Vec<String>> {
    sections
        .iter()
        .find(|(name, _)| name == title)
        .map(|(_, body)| body)
}

/// Lines between the checklist markers (exclusive).
pub fn checklist_block(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line.trim_end() == CHECKLIST_START {
            inside = true;
            continue;
        }
        if line.trim_end() == CHECKLIST_END {
            inside = false;
            continue;
        }
        if inside {
            out.push(line.to_string());
        }
    }
    out
}

/// One checklist item.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckItem {
    pub raw: String,
    pub indent: usize,
    pub depth: usize,
    pub id: String,
    /// Everything after the `**ID**` marker.
    pub text: String,
    pub checked: bool,
    pub well_formed: bool,
}

impl CheckItem {
    /// Checkbox token normalized to `[ ]` — the only byte the agent may change.
    pub fn normalized(&self) -> String {
        format!(
            "{}- [ ] **{}** {}",
            " ".repeat(self.indent),
            self.id,
            self.text
        )
    }
}

/// Parse checklist lines. Tolerant: a malformed line yields `well_formed = false` so the linter and
/// the gate can report it instead of silently skipping it.
pub fn parse_checklist(lines: &[String]) -> Vec<CheckItem> {
    let re = Regex::new(r"^( {4}){0,3}- \[([ x])\] \*\*([0-9]+(?:\.[0-9]+){0,3})\*\* (.*)$")
        .expect("static regex");
    lines
        .iter()
        .map(|raw| {
            if let Some(caps) = re.captures(raw) {
                // measure the indent from the line itself: a repeated capture group only
                // remembers its last repetition, which would pin every depth at 1
                let indent = raw.len() - raw.trim_start_matches(' ').len();
                CheckItem {
                    raw: raw.clone(),
                    indent,
                    depth: indent / 4,
                    id: caps
                        .get(3)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default(),
                    text: caps
                        .get(4)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default(),
                    checked: caps.get(2).map(|m| m.as_str() == "x").unwrap_or(false),
                    well_formed: true,
                }
            } else {
                CheckItem {
                    raw: raw.clone(),
                    indent: raw.len() - raw.trim_start().len(),
                    depth: 0,
                    id: String::new(),
                    text: raw.clone(),
                    checked: false,
                    well_formed: false,
                }
            }
        })
        .collect()
}

/// Leaf flag: an item is a leaf when the next item is not deeper.
pub fn leaf_flags(items: &[CheckItem]) -> Vec<bool> {
    items
        .iter()
        .enumerate()
        .map(|(i, item)| i + 1 == items.len() || items[i + 1].depth <= item.depth)
        .collect()
}

/// First leaf: the first item whose successor is not deeper (or the last item). This is the
/// `CURRENT:` value `progress-init` writes.
pub fn first_leaf(items: &[CheckItem]) -> Option<String> {
    let flags = leaf_flags(items);
    items
        .iter()
        .zip(flags)
        .find(|(_, leaf)| *leaf)
        .map(|(item, _)| item.id.clone())
}

/// Acceptance table rows: `| AC-001 | GWT | `command` | expected |`.
pub fn acceptance_rows(lines: Vec<String>) -> Vec<AcRow> {
    let re = Regex::new(r"^\|\s*(AC-[0-9]+)\s*\|").expect("static regex");
    lines
        .into_iter()
        .filter_map(|line| {
            let id = re.captures(&line)?.get(1)?.as_str().to_string();
            let cells: Vec<&str> = line.split('|').collect();
            // ['', '', gwt, cmd, expected, ...]
            let gwt = cells.get(2).copied().unwrap_or("").trim().to_string();
            let evidence = cells.get(3).copied().unwrap_or("").trim().to_string();
            let expected = cells.get(4).copied().unwrap_or("").trim().to_string();
            Some(AcRow {
                id,
                gwt,
                evidence,
                expected,
            })
        })
        .collect()
}

/// `P-001` style precondition line.
pub fn precondition_id(line: &str) -> Option<String> {
    let re = Regex::new(r"^- \*\*(P-[0-9]+):\*\*").expect("static regex");
    re.captures(line)
        .and_then(|caps| caps.get(1))
        .map(|group| group.as_str().to_string())
}

/// Load a task package from a directory or a `README.md` path.
pub fn load_task(target: &Path) -> anyhow::Result<TaskFile> {
    let dir = if target.is_dir() {
        target
    } else {
        target.parent().unwrap_or(Path::new("."))
    };
    let readme = dir.join("README.md");
    if !readme.is_file() {
        bail!("missing {}", readme.display());
    }
    TaskFile::load(&readme)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
---
schema: task/v4
id: TASK-042
title: \"Some title\"   # inline comment
kind: bugfix
verify: \"taskfmt verify\"
expected_paths:
  - \"src/auth/session/*\"
  - tests/auth/*
---

# TASK-042 — Some title

## Goal

Do the thing.

## Preconditions

- **P-001:** fixture present — `test -f x`
- **P-002:** toolchain — `cargo --version`

## Acceptance criteria

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given s, when a, then b. | `cargo test` | exit 0 |
| AC-002 | Given s, when c, then d. | `cargo test other` | exit 0 |

## Checklist

<!-- checklist:start -->
- [ ] **1** Baseline.
    - [ ] **1.1** Preconditions pass — evidence: commands exit 0.
    - [ ] **1.2** Baseline recorded — evidence: `cargo test`.
- [ ] **2** Work.
    - [ ] **2.1** Unit — evidence: `cargo test`.
- [ ] **3** Acceptance.
    - [ ] **3.1** `AC-001` — evidence: `cargo test`.
    - [ ] **3.2** `AC-002` — evidence: `cargo test`.
    - [ ] **3.3** `AC-001` again — evidence: `cargo test`.
<!-- checklist:end -->
";

    #[test]
    fn parses_frontmatter() {
        let tf = TaskFile::parse(SAMPLE.to_string(), Path::new("README.md")).unwrap();
        assert_eq!(tf.frontmatter.schema, "task/v4");
        assert_eq!(tf.frontmatter.id, "TASK-042");
        assert_eq!(tf.frontmatter.title, "Some title");
        assert_eq!(tf.frontmatter.verify, "taskfmt verify");
        assert_eq!(
            tf.frontmatter.expected_paths,
            vec!["src/auth/session/*", "tests/auth/*"]
        );
    }

    #[test]
    fn parses_sections_and_h1() {
        let tf = TaskFile::parse(SAMPLE.to_string(), Path::new("README.md")).unwrap();
        let titles: Vec<&str> = tf.sections.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            titles,
            vec!["Goal", "Preconditions", "Acceptance criteria", "Checklist"]
        );
        assert_eq!(tf.h1.as_deref(), Some("TASK-042 — Some title"));
        assert_eq!(tf.preconditions.len(), 2);
    }

    #[test]
    fn parses_checklist_and_first_leaf() {
        let tf = TaskFile::parse(SAMPLE.to_string(), Path::new("README.md")).unwrap();
        let items = parse_checklist(&tf.checklist);
        assert_eq!(items.len(), 9);
        assert!(items.iter().all(|item| item.well_formed));
        assert_eq!(items[1].depth, 1);
        assert_eq!(items[1].id, "1.1");
        assert_eq!(first_leaf(&items).as_deref(), Some("1.1"));
        assert_eq!(items[0].normalized(), "- [ ] **1** Baseline.");
    }

    #[test]
    fn parses_acceptance_rows() {
        let tf = TaskFile::parse(SAMPLE.to_string(), Path::new("README.md")).unwrap();
        assert_eq!(tf.ac_rows.len(), 2);
        assert_eq!(tf.ac_rows[0].id, "AC-001");
        assert_eq!(tf.ac_rows[0].evidence, "`cargo test`");
        assert_eq!(tf.ac_rows[1].expected, "exit 0");
    }

    #[test]
    fn flags_malformed_checklist_lines() {
        let items = parse_checklist(&["    - [ ] **3.2X** broken".to_string()]);
        assert!(!items[0].well_formed);
    }

    #[test]
    fn no_frontmatter_is_reported() {
        assert!(TaskFile::parse("no fences here\n".to_string(), Path::new("README.md")).is_err());
    }
}
