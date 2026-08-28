//! `verify.toml` — declarative gate inputs (schema `verify/v1`). Replaces the sourced
//! `verify.config` bash file: data only, no executable content.

use std::path::Path;

use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "verify/v1";
pub const FILE_NAME: &str = "verify.toml";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyConfig {
    pub schema: String,
    /// Scope base. Optional: the gate falls back to TASKFMT_BASE / `--base` / the `baseline` tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_ref: Option<String>,
    #[serde(default)]
    pub focused: CommandGroup,
    #[serde(default)]
    pub regression: CommandGroup,
    #[serde(default)]
    pub lint: CommandGroup,
    #[serde(default)]
    pub forbidden_patterns: Vec<ForbiddenPattern>,
    #[serde(default)]
    pub forbidden_paths: Vec<String>,
    #[serde(default)]
    pub required_paths: Vec<String>,
    /// Scope whitelist: every changed file must match one of these globs.
    #[serde(default)]
    pub allowed_globs: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandGroup {
    #[serde(default)]
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForbiddenPattern {
    pub regex: String,
    /// Relative paths (or files) the regex must not match in. Default: the whole tree.
    #[serde(default)]
    pub paths: Vec<String>,
}

impl VerifyConfig {
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let cfg: VerifyConfig = toml::from_str(text)?;
        if cfg.schema != SCHEMA {
            anyhow::bail!("verify.toml schema is {:?}, want {SCHEMA:?}", cfg.schema);
        }
        Ok(cfg)
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Self::parse(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = include_str!("../testdata/verify-template.toml");

    #[test]
    fn parses_the_template_shape() {
        let cfg = VerifyConfig::parse(TEMPLATE).unwrap();
        assert_eq!(cfg.schema, "verify/v1");
        assert!(cfg.base_ref.is_none());
        assert_eq!(cfg.allowed_globs.len(), 2);
        assert_eq!(cfg.forbidden_patterns.len(), 2);
        assert_eq!(cfg.forbidden_patterns[0].paths, vec!["src", "tests"]);
        assert_eq!(cfg.required_paths, vec!["tests/new_test.rs"]);
        assert_eq!(cfg.focused.commands.len(), 2);
    }

    #[test]
    fn base_ref_is_optional_and_wired_when_present() {
        let cfg = VerifyConfig::parse("schema = \"verify/v1\"\nbase_ref = \"baseline\"\n").unwrap();
        assert_eq!(cfg.base_ref.as_deref(), Some("baseline"));
        assert!(cfg.allowed_globs.is_empty());
    }

    #[test]
    fn wrong_schema_is_rejected() {
        assert!(VerifyConfig::parse("schema = \"verify/v2\"\n").is_err());
    }

    #[test]
    fn misplaced_top_level_keys_are_rejected_not_dropped() {
        // TOML nests every key that follows a table header; a scope whitelist that lands inside
        // a table must fail loudly instead of silently leaving the gate with no allowed globs.
        let misplaced =
            "schema = \"verify/v1\"\n[focused]\ncommands = []\nallowed_globs = [\"*\"]\n";
        let err = VerifyConfig::parse(misplaced).unwrap_err().to_string();
        assert!(err.contains("allowed_globs"), "{err}");
    }

    #[test]
    fn missing_fields_default_to_empty() {
        let cfg = VerifyConfig::parse("schema = \"verify/v1\"\n").unwrap();
        assert!(cfg.focused.commands.is_empty());
        assert!(cfg.forbidden_paths.is_empty());
        assert!(cfg.lint.commands.is_empty());
    }
}
