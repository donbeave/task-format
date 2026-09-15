//! Strict dispatch contract (`execution/v1`).
use serde::Deserialize;
use std::path::Path;

pub const SCHEMA: &str = "execution/v1";
pub const FILE_NAME: &str = "execution.toml";

pub const EFFORT_VALUES: &[&str] = &["low", "medium", "high", "max"];

/// A parse/validation failure with a byte-span-derived TOML coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDiagnostic {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecutionConfig {
    pub schema: String,
    pub profile: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
}

impl ExecutionConfig {
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

    /// Read `execution.toml` when present; missing file is not an error.
    pub fn load_optional(dir: &Path) -> Result<Option<Self>, ConfigDiagnostic> {
        let path = dir.join(FILE_NAME);
        if !path.is_file() {
            return Ok(None);
        }
        Self::parse_located(&std::fs::read_to_string(&path).map_err(|error| ConfigDiagnostic {
            message: error.to_string(),
            line: 1,
            column: 1,
        })?)
        .map(Some)
    }

    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.schema == SCHEMA,
            "execution.toml schema is {:?}, want {SCHEMA:?}",
            self.schema
        );
        anyhow::ensure!(!self.profile.is_empty(), "profile is empty");
        if let Some(effort) = &self.effort {
            anyhow::ensure!(
                EFFORT_VALUES.contains(&effort.as_str()),
                "effort invalid: {effort:?}, want one of {}",
                EFFORT_VALUES.join("|")
            );
        }
        Ok(())
    }

    pub fn validate_against(&self, cfg: &crate::config::ExperimentConfig) -> anyhow::Result<()> {
        cfg.profile(&self.profile)?;
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

fn semantic_location(text: &str, message: &str) -> (usize, usize) {
    let key = ["schema", "profile", "model", "effort"]
        .into_iter()
        .find(|key| message.contains(key));
    toml_key_line(text, key).unwrap_or((1, 1))
}

fn toml_key_line(text: &str, key: Option<&str>) -> Option<(usize, usize)> {
    let lines: Vec<_> = text.lines().collect();
    if let Some(key) = key {
        for (index, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            if trimmed
                .strip_prefix(key)
                .is_some_and(|tail| tail.starts_with(char::is_whitespace) || tail.starts_with('='))
            {
                return Some((index + 1, line.len() - trimmed.len() + 1));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
schema = "execution/v1"
profile = "codex-kimi"
model = "kimi-k3"
effort = "high"
"#;

    #[test]
    fn parses_valid_execution_config() {
        let cfg = ExecutionConfig::parse(VALID).unwrap();
        assert_eq!(cfg.schema, SCHEMA);
        assert_eq!(cfg.profile, "codex-kimi");
        assert_eq!(cfg.model.as_deref(), Some("kimi-k3"));
        assert_eq!(cfg.effort.as_deref(), Some("high"));
    }

    #[test]
    fn rejects_unknown_schema() {
        let text = VALID.replace("execution/v1", "execution/v0");
        let err = ExecutionConfig::parse_located(&text).unwrap_err();
        assert!(err.message.contains("schema"), "{}", err.message);
        assert_eq!(err.line, 2);
    }

    #[test]
    fn rejects_unknown_fields() {
        let text = format!("{VALID}\nextra = true\n");
        let err = ExecutionConfig::parse_located(&text).unwrap_err();
        assert!(
            err.message.contains("unknown field") || err.message.contains("extra"),
            "{}",
            err.message
        );
    }

    #[test]
    fn rejects_invalid_effort() {
        let text = VALID.replace("effort = \"high\"", "effort = \"turbo\"");
        let err = ExecutionConfig::parse_located(&text).unwrap_err();
        assert!(err.message.contains("effort"), "{}", err.message);
        assert_eq!(err.line, 5);
    }

    #[test]
    fn rejects_empty_profile() {
        let text = VALID.replace("profile = \"codex-kimi\"", "profile = \"\"");
        let err = ExecutionConfig::parse_located(&text).unwrap_err();
        assert!(err.message.contains("profile"), "{}", err.message);
    }

    #[test]
    fn load_optional_absent_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(ExecutionConfig::load_optional(dir.path()).unwrap(), None);
    }

    #[test]
    fn load_optional_present_parses() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), VALID).unwrap();
        let cfg = ExecutionConfig::load_optional(dir.path()).unwrap().unwrap();
        assert_eq!(cfg.profile, "codex-kimi");
    }

    #[test]
    fn validate_against_checks_profile_exists() {
        let manifest = r#"
schema = "experiment/v1"
[agents.default]
profile = "p"
[agents.profiles.p]
kind = "claude"
image = "i"
"#;
        let exp = crate::config::ExperimentConfig::parse(manifest).unwrap();
        let cfg = ExecutionConfig::parse(&VALID.replace("codex-kimi", "p")).unwrap();
        assert!(cfg.validate_against(&exp).is_ok());
        let bad = ExecutionConfig::parse(VALID).unwrap();
        assert!(bad.validate_against(&exp).is_err());
    }
}
