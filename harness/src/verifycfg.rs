//! Strict machine contract (`verify/v2`).
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Component, Path};

pub const SCHEMA: &str = "verify/v2";
pub const FILE_NAME: &str = "verify.toml";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyConfig {
    pub schema: String,
    pub task_id: String,
    pub base_tree: String,
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
    pub tree: String,
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
        let cfg: Self = toml::from_str(text)?;
        cfg.validate()?;
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
        anyhow::ensure!(oid(&self.base_tree), "base_tree invalid");
        if let Some(p) = &self.predecessor {
            anyhow::ensure!(
                valid_task(&p.task_id) && p.task_id != self.task_id && oid(&p.tree),
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
            if c.phase == Phase::Gate {
                gate += 1;
            }
        }
        anyhow::ensure!(gate == 1, "exactly one gate check required");
        Ok(())
    }
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
