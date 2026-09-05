//! Strict machine contract (`verify/v2`).
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Component, Path};

pub const SCHEMA: &str = "verify/v2";
pub const FILE_NAME: &str = "verify.toml";

/// A verifier parse/validation failure with a byte-span-derived TOML coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDiagnostic {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyConfig {
    pub schema: String,
    pub task_id: String,
    /// Standalone verification may pin an immutable base. Lifecycle gates always pass the base
    /// recorded for their run, so task packages never claim a future predecessor tree.
    #[serde(default)]
    pub base_tree: Option<String>,
    #[serde(default)]
    pub predecessor: Option<Predecessor>,
    pub writable_paths: Vec<String>,
    #[serde(default)]
    pub forbidden_paths: Vec<String>,
    #[serde(default)]
    pub forbidden_patterns: Vec<ForbiddenPattern>,
    pub checks: Vec<Check>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Predecessor {
    pub task_id: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForbiddenPattern {
    pub regex: String,
    #[serde(default)]
    pub paths: Vec<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub id: String,
    pub phase: Phase,
    #[serde(default)]
    pub argv: Option<Vec<String>>,
    #[serde(default)]
    pub shell: Option<String>,
    #[serde(default)]
    pub requirements: Vec<String>,
    #[serde(default)]
    pub acceptance: Vec<String>,
    #[serde(default)]
    pub expected: Expected,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    pub exit: Option<i32>,
    #[serde(default)]
    pub stdout_contains: Vec<String>,
    #[serde(default)]
    pub stdout_excludes: Vec<String>,
    #[serde(default)]
    pub stdout_regex: Vec<String>,
    #[serde(default)]
    pub stderr_contains: Vec<String>,
    #[serde(default)]
    pub stderr_excludes: Vec<String>,
    #[serde(default)]
    pub stderr_regex: Vec<String>,
    #[serde(default)]
    pub stdout_occurrences: Vec<Occurrence>,
    #[serde(default)]
    pub stderr_occurrences: Vec<Occurrence>,
    #[serde(default)]
    pub required_artifacts: Vec<String>,
    #[serde(default)]
    pub forbidden_artifacts: Vec<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Occurrence {
    pub text: String,
    pub count: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Precondition,
    Focused,
    Regression,
    Lint,
    Gate,
}
impl VerifyConfig {
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        Self::parse_located(text).map_err(|error| anyhow::anyhow!(error.message))
    }
    pub fn parse_located(text: &str) -> Result<Self, ConfigDiagnostic> {
        let cfg: Self = toml::from_str(text).map_err(|error| {
            let (line, column) = error
                .span()
                .map(|span| line_column(text, span.start))
                .unwrap_or((1, 1));
            ConfigDiagnostic {
                message: error.to_string(),
                line,
                column,
            }
        })?;
        cfg.validate().map_err(|error| {
            let message = format!("{error:#}");
            let (line, column) = semantic_location(text, &message);
            ConfigDiagnostic {
                message,
                line,
                column,
            }
        })?;
        Ok(cfg)
    }
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        Self::parse(&std::fs::read_to_string(path)?)
    }
    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.schema == SCHEMA,
            "verify.toml schema is {:?}, want {SCHEMA:?}",
            self.schema
        );
        anyhow::ensure!(valid_task(&self.task_id), "task_id invalid");
        if let Some(base_tree) = &self.base_tree {
            anyhow::ensure!(oid(base_tree), "base_tree invalid");
        }
        if let Some(p) = &self.predecessor {
            anyhow::ensure!(
                valid_task(&p.task_id) && p.task_id != self.task_id,
                "predecessor invalid"
            );
        }
        unique_paths("writable_paths", &self.writable_paths)?;
        unique_paths("forbidden_paths", &self.forbidden_paths)?;
        anyhow::ensure!(!self.writable_paths.is_empty(), "writable_paths empty");
        anyhow::ensure!(!self.checks.is_empty(), "checks empty");
        let mut ids = BTreeSet::new();
        let mut gate = 0;
        for c in &self.checks {
            anyhow::ensure!(id(&c.id, "CHK-"), "check id invalid: {}", c.id);
            anyhow::ensure!(ids.insert(&c.id), "duplicate check id: {}", c.id);
            anyhow::ensure!(
                c.argv.is_some() != c.shell.is_some(),
                "{} needs exactly one of argv or shell",
                c.id
            );
            if let Some(a) = &c.argv {
                anyhow::ensure!(
                    !a.is_empty() && a.iter().all(|v| !v.is_empty()),
                    "{} argv empty",
                    c.id
                )
            };
            if let Some(s) = &c.shell {
                anyhow::ensure!(!s.trim().is_empty(), "{} shell empty", c.id)
            };
            unique_ids("requirements", &c.requirements, "R-")?;
            unique_ids("acceptance", &c.acceptance, "AC-")?;
            validate_expected(&c.id, &c.expected)?;
            if c.phase == Phase::Gate {
                gate += 1;
            }
        }
        anyhow::ensure!(gate == 1, "exactly one gate check required");
        Ok(())
    }
}

fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let prefix = &text[..offset.min(text.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.chars().count() + 1, |(_, tail)| {
            tail.chars().count() + 1
        });
    (line, column)
}

/// Locate semantic errors from TOML's own key/table grammar. This never scans arbitrary values:
/// repeated words in commands or expected output cannot redirect a config diagnostic.
fn semantic_location(text: &str, message: &str) -> (usize, usize) {
    let key = [
        "schema",
        "task_id",
        "base_tree",
        "predecessor",
        "writable_paths",
        "forbidden_paths",
        "forbidden_patterns",
        "requirements",
        "acceptance",
        "argv",
        "shell",
        "phase",
        "id",
        "stdout_regex",
        "stderr_regex",
        "stdout_occurrences",
        "stderr_occurrences",
        "required_artifacts",
        "forbidden_artifacts",
    ]
    .into_iter()
    .find(|key| message.contains(key));
    let check = Regex::new(r"\bCHK-[0-9]+\b")
        .unwrap()
        .find(message)
        .map(|m| m.as_str());
    toml_key_line(text, key, check).unwrap_or((1, 1))
}

fn toml_key_line(text: &str, key: Option<&str>, check: Option<&str>) -> Option<(usize, usize)> {
    let lines: Vec<_> = text.lines().collect();
    let mut selected = None;
    if let Some(check) = check {
        for (index, line) in lines.iter().enumerate() {
            if line.trim() == "[[checks]]" {
                let end = lines[index + 1..]
                    .iter()
                    .position(|next| next.trim().starts_with("[[") || next.trim().starts_with('['))
                    .map(|relative| index + 1 + relative)
                    .unwrap_or(lines.len());
                if lines[index + 1..end].iter().any(|candidate| {
                    candidate.trim_start().starts_with("id") && candidate.contains(check)
                }) {
                    selected = Some((index, end));
                    break;
                }
            }
        }
    }
    let (start, end) = selected.unwrap_or((0, lines.len()));
    if let Some(key) = key {
        for (index, line) in lines[start..end].iter().enumerate() {
            let trimmed = line.trim_start();
            if trimmed
                .strip_prefix(key)
                .is_some_and(|tail| tail.starts_with(char::is_whitespace) || tail.starts_with('='))
            {
                return Some((start + index + 1, line.len() - trimmed.len() + 1));
            }
        }
    }
    selected.map(|(start, _)| (start + 1, 1))
}
fn validate_expected(id: &str, expected: &Expected) -> anyhow::Result<()> {
    for (name, patterns) in [
        ("stdout_regex", &expected.stdout_regex),
        ("stderr_regex", &expected.stderr_regex),
    ] {
        for pattern in patterns {
            Regex::new(pattern)
                .map_err(|e| anyhow::anyhow!("{id} {name} invalid regex {pattern:?}: {e}"))?;
        }
    }
    for (name, entries) in [
        ("stdout_occurrences", &expected.stdout_occurrences),
        ("stderr_occurrences", &expected.stderr_occurrences),
    ] {
        let mut seen = BTreeSet::new();
        for entry in entries {
            anyhow::ensure!(
                !entry.text.is_empty() && seen.insert(&entry.text),
                "{id} {name} has empty or duplicate text"
            );
        }
    }
    unique_paths("required_artifacts", &expected.required_artifacts)?;
    unique_paths("forbidden_artifacts", &expected.forbidden_artifacts)?;
    let required: BTreeSet<_> = expected.required_artifacts.iter().collect();
    anyhow::ensure!(
        expected
            .forbidden_artifacts
            .iter()
            .all(|p| !required.contains(p)),
        "{id} artifact both required and forbidden"
    );
    Ok(())
}
fn id(s: &str, p: &str) -> bool {
    s.strip_prefix(p)
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}
fn valid_task(s: &str) -> bool {
    id(s, "TASK-")
}
fn oid(s: &str) -> bool {
    (s.len() == 40 || s.len() == 64) && s.chars().all(|c| c.is_ascii_hexdigit())
}
fn unique_ids(n: &str, v: &[String], p: &str) -> anyhow::Result<()> {
    let mut x = BTreeSet::new();
    for s in v {
        anyhow::ensure!(id(s, p) && x.insert(s), "{n} invalid or duplicate: {s}");
    }
    Ok(())
}
fn unique_paths(n: &str, v: &[String]) -> anyhow::Result<()> {
    let mut x = BTreeSet::new();
    for s in v {
        let p = Path::new(s);
        anyhow::ensure!(
            !s.is_empty()
                && !p.is_absolute()
                && p.components().all(|c| matches!(c, Component::Normal(_)))
                && x.insert(s),
            "{n} unsafe or duplicate: {s}"
        );
    }
    Ok(())
}
