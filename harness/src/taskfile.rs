//! `README.md` task-file parser: strict `task/v5` frontmatter, H2 sections, and the checklist
//! block. Hand-parsed on purpose — the frontmatter is a small fixed schema and the checklist
//! grammar is bespoke.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail, ensure};
use regex::Regex;

use crate::acceptance::{self, AcceptanceDocument};

pub const CHECKLIST_START: &str = "<!-- checklist:start -->";
pub const CHECKLIST_END: &str = "<!-- checklist:end -->";

/// Frontmatter keys the task contract uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frontmatter {
    pub schema: String,
    pub id: String,
    pub title: String,
    pub kind: String,
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
    /// Typed AC blocks in `## Acceptance criteria` only.
    pub typed_acceptance: AcceptanceDocument,
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
        let frontmatter = parse_frontmatter(&raw_fm)?;
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
        // Acceptance is deliberately section-bounded. A stray `### AC-*` elsewhere in the
        // README is prose, not an alternate source of acceptance authority.
        let acceptance_text = section_body(&sections, "Acceptance criteria")
            .map(|body| body.join("\n"))
            .unwrap_or_default();
        let typed_acceptance = acceptance::parse(&acceptance_text);
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
            typed_acceptance,
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

    /// One-based source line for a frontmatter key.  This is deliberately parsed from the
    /// frontmatter grammar, rather than searched in prose later by the linter.
    pub fn frontmatter_key_line(&self, key: &str) -> Option<usize> {
        self.text
            .lines()
            .enumerate()
            .take_while(|(index, line)| *index == 0 || line.trim_end() != "---")
            .find_map(|(index, line)| {
                (index > 0
                    && line
                        .strip_prefix(key)
                        .is_some_and(|rest| rest.starts_with(':')))
                .then_some(index + 1)
            })
    }

    /// One-based source line of the exact H2 heading.
    pub fn section_line(&self, title: &str) -> Option<usize> {
        let heading = format!("## {title}");
        self.text
            .lines()
            .position(|line| line.trim_end() == heading)
            .map(|line| line + 1)
    }

    pub fn h1_line(&self) -> Option<usize> {
        self.text
            .lines()
            .position(|line| line.trim_end().starts_with("# "))
            .map(|line| line + 1)
    }

    /// One-based line for an ID declaration in its owning Markdown section.
    pub fn definition_line(&self, section: &str, id: &str) -> Option<usize> {
        let start = self.section_line(section)?;
        let declaration = match section {
            "Requirements" => format!("- **{id}"),
            "Acceptance criteria" => format!("### {id}"),
            _ => return None,
        };
        self.text
            .lines()
            .enumerate()
            .skip(start)
            .take_while(|(_, line)| !line.trim_end().starts_with("## "))
            .find_map(|(line, text)| {
                text.trim_start()
                    .starts_with(&declaration)
                    .then_some(line + 1)
            })
    }

    /// One-based line in the README from a line relative to the acceptance section body.
    pub fn acceptance_line(&self, relative: usize) -> Option<usize> {
        self.section_line("Acceptance criteria")
            .map(|heading| heading + relative)
    }

    pub fn checklist_line(&self, id_or_raw: &str) -> Option<usize> {
        let start = self
            .text
            .lines()
            .position(|line| line.trim_end() == CHECKLIST_START)?;
        self.text
            .lines()
            .enumerate()
            .skip(start)
            .take_while(|(_, line)| line.trim_end() != CHECKLIST_END)
            .find_map(|(line, text)| text.contains(id_or_raw).then_some(line + 1))
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

/// Parse the entire current frontmatter schema. There are no optional keys, comments, lists, or
/// compatibility aliases: `verify.toml` owns machine configuration.
pub fn parse_frontmatter(fm: &str) -> anyhow::Result<Frontmatter> {
    const KEYS: [&str; 4] = ["schema", "id", "title", "kind"];
    let mut values = BTreeMap::new();

    for (line_number, line) in fm.lines().enumerate() {
        let line_number = line_number + 2; // account for the opening fence
        ensure!(
            !line.trim().is_empty(),
            "frontmatter line {line_number}: blank lines are not allowed"
        );
        ensure!(
            line == line.trim(),
            "frontmatter line {line_number}: indentation or trailing whitespace is not allowed"
        );
        let (key, raw_value) = line.split_once(':').ok_or_else(|| {
            anyhow::anyhow!("frontmatter line {line_number}: expected `key: value`")
        })?;
        ensure!(
            KEYS.contains(&key),
            "frontmatter line {line_number}: unknown or malformed key `{key}`"
        );
        ensure!(
            raw_value.starts_with(' ') && !raw_value.starts_with("  "),
            "frontmatter line {line_number}: expected one space after `:`"
        );
        let value = raw_value
            .strip_prefix(' ')
            .expect("validated one leading space");
        ensure!(
            !value.is_empty(),
            "frontmatter line {line_number}: `{key}` must not be empty"
        );
        let value = if value.starts_with('"') || value.ends_with('"') {
            value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "frontmatter line {line_number}: malformed quoted value for `{key}`"
                    )
                })?
                .to_string()
        } else {
            ensure!(
                !value.contains('"') && !value.contains('#'),
                "frontmatter line {line_number}: malformed scalar value for `{key}`"
            );
            value.to_string()
        };
        ensure!(
            values.insert(key, value).is_none(),
            "frontmatter line {line_number}: duplicate key `{key}`"
        );
    }

    let mut required = |key| {
        values
            .remove(key)
            .ok_or_else(|| anyhow::anyhow!("frontmatter: missing required key `{key}`"))
    };
    let schema = required("schema")?;
    ensure!(
        schema == "task/v5",
        "frontmatter: expected schema `task/v5`, got `{schema}`"
    );
    Ok(Frontmatter {
        schema,
        id: required("id")?,
        title: required("title")?,
        kind: required("kind")?,
    })
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
    // Indentation is four-space units but hierarchy depth and dotted IDs are intentionally
    // unbounded; neither a leaf count nor a single-child shape affects task correctness.
    let re = Regex::new(r"^(?: {4})*- \[([ x])\] \*\*([0-9]+(?:\.[0-9]+)*)\*\* (.*)$")
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
                        .get(2)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default(),
                    text: caps
                        .get(3)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default(),
                    checked: caps.get(1).map(|m| m.as_str() == "x").unwrap_or(false),
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

/// Content of the first inline-code span in `text` (`` `cargo test` `` → `cargo test`).
pub fn first_code_span(text: &str) -> Option<&str> {
    let start = text.find('`')?;
    let rest = &text[start + 1..];
    let end = rest.find('`')?;
    let span = &rest[..end];
    if span.is_empty() { None } else { Some(span) }
}

/// Evidence text of a checklist item: everything after the first `evidence:`, trimmed, without a
/// trailing period. `None` when the item states no evidence.
pub fn leaf_evidence(item: &CheckItem) -> Option<String> {
    let (_, after) = item.text.split_once("evidence:")?;
    let trimmed = after.trim();
    Some(trimmed.strip_suffix('.').unwrap_or(trimmed).to_string())
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
schema: task/v5
id: TASK-042
title: \"Some title\"
kind: bugfix
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
        assert_eq!(tf.frontmatter.schema, "task/v5");
        assert_eq!(tf.frontmatter.id, "TASK-042");
        assert_eq!(tf.frontmatter.title, "Some title");
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
    fn acceptance_parse_is_bounded_to_acceptance_section() {
        let tf = TaskFile::parse(SAMPLE.to_string(), Path::new("README.md")).unwrap();
        assert!(!tf.typed_acceptance.detected);
        let text = format!("{SAMPLE}\n## Notes\n\n### AC-999 — prose only\n");
        let tf = TaskFile::parse(text, Path::new("README.md")).unwrap();
        assert!(!tf.typed_acceptance.detected);
    }

    #[test]
    fn flags_malformed_checklist_lines() {
        let items = parse_checklist(&["    - [ ] **3.2X** broken".to_string()]);
        assert!(!items[0].well_formed);
    }

    #[test]
    fn first_code_span_and_leaf_evidence() {
        assert_eq!(
            first_code_span("run `cargo test -p auth` then `x`"),
            Some("cargo test -p auth")
        );
        assert_eq!(first_code_span("no span"), None);
        assert_eq!(first_code_span("empty `` span"), None);
        let items = parse_checklist(&[
            "    - [ ] **2.1** Unit — evidence: `cargo test` exits 0.".to_string(),
            "    - [ ] **2.2** Unit without evidence".to_string(),
            "    - [ ] **2.3** Empty — evidence: ".to_string(),
        ]);
        assert_eq!(
            leaf_evidence(&items[0]).as_deref(),
            Some("`cargo test` exits 0")
        );
        assert_eq!(leaf_evidence(&items[1]), None);
        assert_eq!(leaf_evidence(&items[2]).as_deref(), Some(""));
    }

    #[test]
    fn no_frontmatter_is_reported() {
        assert!(TaskFile::parse("no fences here\n".to_string(), Path::new("README.md")).is_err());
    }

    #[test]
    fn strict_frontmatter_rejects_unknown_duplicate_and_malformed_keys() {
        for frontmatter in [
            "schema: task/v5\nid: TASK-042\ntitle: Some title\nkind: bugfix\nverify: no",
            "schema: task/v5\nid: TASK-042\nid: TASK-043\ntitle: Some title\nkind: bugfix",
            "schema: task/v5\nid TASK-042\ntitle: Some title\nkind: bugfix",
            "schema: task/v5\nid: TASK-042\ntitle: Some title",
            "schema: task/v4\nid: TASK-042\ntitle: Some title\nkind: bugfix",
        ] {
            let text = format!("---\n{frontmatter}\n---\n");
            assert!(
                TaskFile::parse(text, Path::new("README.md")).is_err(),
                "{frontmatter}"
            );
        }
    }
}
