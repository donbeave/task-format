//! `experiment.toml` — the experiment manifest (schema `experiment/v1`).
//!
//! Everything except `[agents]` has a default, so a minimal manifest is viable; the agent profiles
//! carry the secret references and therefore must always be explicit.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "experiment/v1";

/// The manifest's file name, and the repo-relative path `--config` defaults to.
pub const MANIFEST_NAME: &str = "experiment.toml";

/// Walk `start` and its ancestors looking for `rel`; return the first hit and every directory
/// searched, in order.
///
/// The manifest belongs to a checkout, not to the process's working directory. Resolving a
/// repo-relative default against the cwd made every command that needs the manifest — including
/// read-only, config-independent ones like `attach`, `status` and `gate` — usable only from the
/// repo root. Discovery removes that coupling instead of exempting individual commands.
pub fn discover_upward(start: &Path, rel: &Path) -> (Option<PathBuf>, Vec<PathBuf>) {
    let mut searched = Vec::new();
    for dir in start.ancestors() {
        searched.push(dir.to_path_buf());
        let candidate = dir.join(rel);
        if candidate.is_file() {
            return (Some(candidate), searched);
        }
    }
    (None, searched)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentConfig {
    pub schema: String,
    #[serde(default)]
    pub github: Github,
    #[serde(default)]
    pub paths: Paths,
    #[serde(default)]
    pub images: Images,
    #[serde(default)]
    pub runtime: Runtime,
    #[serde(default)]
    pub prereq: Prereq,
    pub agents: Agents,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Github {
    #[serde(default = "default_owner")]
    pub owner: String,
    #[serde(default = "default_repo_prefix")]
    pub repo_prefix: String,
}

fn default_owner() -> String {
    "donbeave".to_string()
}

fn default_repo_prefix() -> String {
    "taskfmt-experiment".to_string()
}

impl Default for Github {
    fn default() -> Self {
        Self {
            owner: default_owner(),
            repo_prefix: default_repo_prefix(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paths {
    #[serde(default = "default_tasks_dir")]
    pub tasks_dir: String,
    #[serde(default = "default_runs_dir")]
    pub runs_dir: String,
    #[serde(default = "default_seed_dir")]
    pub seed_dir: String,
    #[serde(default = "default_template_dir")]
    pub template_dir: String,
    #[serde(default = "default_goal_prompt")]
    pub goal_prompt: String,
}

fn default_tasks_dir() -> String {
    "experiments/tasks".to_string()
}
fn default_runs_dir() -> String {
    "experiments/runs".to_string()
}
fn default_seed_dir() -> String {
    "experiments/fixtures/seed".to_string()
}
fn default_template_dir() -> String {
    "reference/task-template".to_string()
}
fn default_goal_prompt() -> String {
    "harness/goal-prompt.md".to_string()
}

impl Default for Paths {
    fn default() -> Self {
        Self {
            tasks_dir: default_tasks_dir(),
            runs_dir: default_runs_dir(),
            seed_dir: default_seed_dir(),
            template_dir: default_template_dir(),
            goal_prompt: default_goal_prompt(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Images {
    #[serde(default = "default_image_taskfmt")]
    pub taskfmt: String,
    #[serde(default = "default_image_base")]
    pub base: String,
    #[serde(default = "default_image_claude")]
    pub claude: String,
    #[serde(default = "default_image_codex")]
    pub codex: String,
}

fn default_image_taskfmt() -> String {
    "harness-taskfmt:latest".to_string()
}
fn default_image_base() -> String {
    "harness-base:latest".to_string()
}
fn default_image_claude() -> String {
    "harness-claude:latest".to_string()
}
fn default_image_codex() -> String {
    "harness-codex:latest".to_string()
}

impl Default for Images {
    fn default() -> Self {
        Self {
            taskfmt: default_image_taskfmt(),
            base: default_image_base(),
            claude: default_image_claude(),
            codex: default_image_codex(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Runtime {
    #[serde(default = "default_memory")]
    pub memory: String,
    #[serde(default = "default_cpus")]
    pub cpus: f32,
    #[serde(default = "default_pids_limit")]
    pub pids_limit: i64,
    #[serde(default = "default_prereq_timeout_s")]
    pub prereq_timeout_s: u64,
    #[serde(default = "default_kill_after_min")]
    pub kill_after_min: u64,
}

fn default_memory() -> String {
    "4g".to_string()
}
fn default_cpus() -> f32 {
    2.0
}
fn default_pids_limit() -> i64 {
    2048
}
fn default_prereq_timeout_s() -> u64 {
    180
}
fn default_kill_after_min() -> u64 {
    90
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            memory: default_memory(),
            cpus: default_cpus(),
            pids_limit: default_pids_limit(),
            prereq_timeout_s: default_prereq_timeout_s(),
            kill_after_min: default_kill_after_min(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prereq {
    #[serde(default = "default_pg_image")]
    pub image: String,
    #[serde(default = "default_pg_user")]
    pub user: String,
    #[serde(default = "default_pg_password")]
    pub password: String,
    #[serde(default = "default_pg_db")]
    pub db: String,
    #[serde(default = "default_pg_port")]
    pub port: u16,
}

fn default_pg_image() -> String {
    "postgres:16-alpine".to_string()
}
fn default_pg_user() -> String {
    "pgtui".to_string()
}
fn default_pg_password() -> String {
    "pgtui".to_string()
}
fn default_pg_db() -> String {
    "pgtui".to_string()
}
fn default_pg_port() -> u16 {
    5432
}

impl Default for Prereq {
    fn default() -> Self {
        Self {
            image: default_pg_image(),
            user: default_pg_user(),
            password: default_pg_password(),
            db: default_pg_db(),
            port: default_pg_port(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agents {
    /// Profile used when `--agent` is not given: `[agents.default] profile = "zai-flash"`.
    #[serde(default)]
    pub default: AgentDefault,
    pub profiles: BTreeMap<String, AgentProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentDefault {
    #[serde(default)]
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub kind: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_effort")]
    pub effort: String,
    pub image: String,
    #[serde(default)]
    pub env_static: BTreeMap<String, String>,
    /// Secret **references**, resolved at dispatch time only — `file://NAME` (a 0600 file directly
    /// inside `$HOME/.config/taskfmt/`) or `op://…` (`op read`); see [`crate::ops::op`].
    /// The reference may be committed; the resolved value may never be.
    #[serde(default)]
    pub env_secret: BTreeMap<String, String>,
}

fn default_effort() -> String {
    "high".to_string()
}

impl ExperimentConfig {
    /// Parse a manifest from text.
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let cfg: ExperimentConfig =
            toml::from_str(text).context("experiment.toml does not parse")?;
        if cfg.schema != SCHEMA {
            bail!(
                "experiment.toml schema is {:?}, want {SCHEMA:?}",
                cfg.schema
            );
        }
        if cfg.agents.default.profile.is_empty() {
            bail!("experiment.toml: agents.default.profile is empty");
        }
        if !cfg
            .agents
            .profiles
            .contains_key(&cfg.agents.default.profile)
        {
            bail!(
                "experiment.toml: agents.default.profile {:?} has no [agents.profiles.{}] table",
                cfg.agents.default.profile,
                cfg.agents.default.profile
            );
        }
        for (name, profile) in &cfg.agents.profiles {
            if profile.kind != "claude" && profile.kind != "codex" {
                bail!(
                    "experiment.toml: profile {name} has kind {:?}, want claude|codex",
                    profile.kind
                );
            }
        }
        Ok(cfg)
    }

    /// Resolve the path a manifest is read from.
    ///
    /// Absolute: taken as given. An explicit `--config` (or `$TASKFMT_CONFIG`) is made absolute
    /// before it reaches here, so absolute means "the operator named this file" and keeps the
    /// original semantics exactly. Relative: the repo-relative default, discovered by walking up
    /// from the current directory.
    pub fn resolve_path(path: &Path) -> anyhow::Result<PathBuf> {
        if path.is_absolute() {
            return Ok(path.to_path_buf());
        }
        let cwd = std::env::current_dir().context("cannot read the current directory")?;
        Self::resolve_path_from(&cwd, path)
    }

    /// `resolve_path` with the starting directory passed in (the cwd-free half, so it is testable
    /// without mutating process state).
    pub fn resolve_path_from(start: &Path, path: &Path) -> anyhow::Result<PathBuf> {
        if path.is_absolute() {
            return Ok(path.to_path_buf());
        }
        let (found, searched) = discover_upward(start, path);
        found.ok_or_else(|| {
            anyhow!(
                "cannot find the experiment manifest {} in {} or any ancestor; searched: {}. \
                 Run from inside the task-format checkout or pass --config <path>",
                path.display(),
                start.display(),
                searched
                    .iter()
                    .map(|dir| dir.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
    }

    /// Load a manifest and resolve the repository root it lives in. The root is the manifest's own
    /// directory, so `Resolved`'s `paths.*` follow the manifest discovery found, never the cwd.
    pub fn load(path: &Path) -> anyhow::Result<(Self, PathBuf)> {
        let path = Self::resolve_path(path)?;
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read experiment manifest {}", path.display()))?;
        let root = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let cfg = Self::parse(&text).with_context(|| format!("in {}", path.display()))?;
        Ok((cfg, root))
    }

    pub fn profile(&self, name: &str) -> anyhow::Result<&AgentProfile> {
        self.agents
            .profiles
            .get(name)
            .with_context(|| format!("no agent profile named {name:?} in experiment.toml"))
    }

    pub fn default_profile(&self) -> &str {
        &self.agents.default.profile
    }
}

/// The manifest plus every path it names, resolved against the manifest's directory.
pub struct Resolved {
    pub root: PathBuf,
    pub cfg: ExperimentConfig,
}

impl Resolved {
    pub fn new(root: &Path, cfg: ExperimentConfig) -> Self {
        Self {
            root: root.to_path_buf(),
            cfg,
        }
    }

    fn join(&self, rel: &str) -> PathBuf {
        let p = Path::new(rel);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.root.join(p)
        }
    }

    pub fn tasks_dir(&self) -> PathBuf {
        self.join(&self.cfg.paths.tasks_dir)
    }
    pub fn runs_dir(&self) -> PathBuf {
        self.join(&self.cfg.paths.runs_dir)
    }
    pub fn seed_dir(&self) -> PathBuf {
        self.join(&self.cfg.paths.seed_dir)
    }
    pub fn template_dir(&self) -> PathBuf {
        self.join(&self.cfg.paths.template_dir)
    }
    pub fn goal_prompt(&self) -> PathBuf {
        self.join(&self.cfg.paths.goal_prompt)
    }
    /// `runs_dir/<id>`
    pub fn run_dir(&self, run_id: &str) -> PathBuf {
        self.runs_dir().join(run_id)
    }
    /// `runs_dir/<id>/experiment.json`
    pub fn experiment_file(&self, id: &str) -> PathBuf {
        self.run_dir(id).join("experiment.json")
    }
    /// The harness folder (crate + image assets).
    pub fn harness_dir(&self) -> PathBuf {
        self.join("harness")
    }
}

/// `YYYYMMDD-HHMMSS` in UTC, used for run ids and repo names.
pub fn timestamp_compact() -> String {
    chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string()
}

/// RFC 3339 UTC timestamp with second precision, used inside manifests.
pub fn timestamp_rfc3339() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"
schema = "experiment/v1"
[github]
owner = "donbeave"
repo_prefix = "taskfmt-experiment"
[paths]
tasks_dir = "experiments/tasks"
runs_dir = "experiments/runs"
seed_dir = "experiments/fixtures/seed"
template_dir = "reference/task-template"
goal_prompt = "harness/goal-prompt.md"
[images]
taskfmt = "harness-taskfmt:latest"
base = "harness-base:latest"
claude = "harness-claude:latest"
codex = "harness-codex:latest"
[runtime]
memory = "4g"
cpus = 2
pids_limit = 2048
prereq_timeout_s = 180
kill_after_min = 90
[prereq]
image = "postgres:16-alpine"
user = "pgtui"
password = "pgtui"
db = "pgtui"
port = 5432
[agents.default]
profile = "zai-flash"
[agents.profiles.zai-flash]
kind = "claude"
model = "glm-5.3-flash"
effort = "low"
image = "harness-claude:latest"
[agents.profiles.zai-flash.env_static]
ANTHROPIC_BASE_URL = "https://api.z.ai/api/anthropic"
[agents.profiles.zai-flash.env_secret]
ANTHROPIC_AUTH_TOKEN = "op://vault/item/section/field"
"#;

    #[test]
    fn parses_the_documented_manifest() {
        let cfg = ExperimentConfig::parse(EXAMPLE).unwrap();
        assert_eq!(cfg.github.owner, "donbeave");
        assert_eq!(cfg.runtime.kill_after_min, 90);
        assert_eq!(cfg.prereq.port, 5432);
        assert_eq!(cfg.default_profile(), "zai-flash");
        let p = cfg.profile("zai-flash").unwrap();
        assert_eq!(p.kind, "claude");
        assert_eq!(
            p.env_static["ANTHROPIC_BASE_URL"],
            "https://api.z.ai/api/anthropic"
        );
        assert!(p.env_secret["ANTHROPIC_AUTH_TOKEN"].starts_with("op://"));
    }

    #[test]
    fn sections_default() {
        let cfg = ExperimentConfig::parse(
            "schema = \"experiment/v1\"\n[agents.default]\nprofile = \"p\"\n[agents.profiles.p]\nkind = \"claude\"\nimage = \"i\"\n",
        )
        .unwrap();
        assert_eq!(cfg.paths.tasks_dir, "experiments/tasks");
        assert_eq!(cfg.runtime.prereq_timeout_s, 180);
        assert_eq!(cfg.github.repo_prefix, "taskfmt-experiment");
        assert_eq!(cfg.profile("p").unwrap().effort, "high");
    }

    #[test]
    fn rejects_bad_schema_and_broken_agents() {
        let err = ExperimentConfig::parse(&EXAMPLE.replace("experiment/v1", "experiment/v2"))
            .unwrap_err();
        assert!(err.to_string().contains("schema"), "{err:#}");
        let no_agents = "schema = \"experiment/v1\"\n";
        assert!(ExperimentConfig::parse(no_agents).is_err());
        let unknown_default = "schema = \"experiment/v1\"\n[agents.default]\nprofile = \"nope\"\n[agents.profiles.p]\nkind = \"claude\"\nimage = \"i\"\n";
        let err = ExperimentConfig::parse(unknown_default).unwrap_err();
        assert!(err.to_string().contains("nope"), "{err:#}");
        let bad_kind = "schema = \"experiment/v1\"\n[agents.default]\nprofile = \"p\"\n[agents.profiles.p]\nkind = \"ghost\"\nimage = \"i\"\n";
        assert!(ExperimentConfig::parse(bad_kind).is_err());
    }

    #[test]
    fn missing_field_is_reported() {
        let text =
            "schema = \"experiment/v1\"\n[agents.default]\nprofile = \"p\"\n[agents.profiles.p]\n";
        let err = ExperimentConfig::parse(text).unwrap_err();
        assert!(
            format!("{err:#}").contains("image") || format!("{err:#}").contains("missing field"),
            "{err:#}"
        );
    }

    #[test]
    fn manifest_is_discovered_in_the_start_dir() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::write(root.join(MANIFEST_NAME), EXAMPLE).unwrap();
        let found = ExperimentConfig::resolve_path_from(&root, Path::new(MANIFEST_NAME)).unwrap();
        assert_eq!(found, root.join(MANIFEST_NAME));
        // and the root the manifest resolves paths against is its own directory, not the start dir
        let (_, resolved_root) = ExperimentConfig::load(&found).unwrap();
        assert_eq!(resolved_root, root);
    }

    #[test]
    fn manifest_is_discovered_in_an_ancestor() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::write(root.join(MANIFEST_NAME), EXAMPLE).unwrap();
        let deep = root.join("experiments").join("runs").join("some-run");
        std::fs::create_dir_all(&deep).unwrap();
        let found = ExperimentConfig::resolve_path_from(&deep, Path::new(MANIFEST_NAME)).unwrap();
        assert_eq!(found, root.join(MANIFEST_NAME));
        // paths.* stay anchored on the manifest's directory, so a subdirectory start is harmless
        let (cfg, resolved_root) = ExperimentConfig::load(&found).unwrap();
        assert_eq!(resolved_root, root);
        assert_eq!(
            Resolved::new(&resolved_root, cfg).runs_dir(),
            root.join("experiments/runs")
        );
    }

    #[test]
    fn a_missing_manifest_names_every_directory_searched() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let deep = root.join("a").join("b");
        std::fs::create_dir_all(&deep).unwrap();
        let err = ExperimentConfig::resolve_path_from(&deep, Path::new(MANIFEST_NAME)).unwrap_err();
        let msg = format!("{err:#}");
        for searched in [deep.as_path(), root.join("a").as_path(), root.as_path()] {
            assert!(
                msg.contains(&searched.display().to_string()),
                "{searched:?} missing from {msg}"
            );
        }
        assert!(msg.contains("--config <path>"), "{msg}");
    }

    #[test]
    fn an_absolute_manifest_path_bypasses_discovery() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::write(root.join(MANIFEST_NAME), EXAMPLE).unwrap();
        let deep = root.join("sub");
        std::fs::create_dir_all(&deep).unwrap();
        // an explicit path is used as given, even when it does not exist: no ancestor is consulted
        let named = deep.join("elsewhere.toml");
        assert_eq!(
            ExperimentConfig::resolve_path_from(&deep, &named).unwrap(),
            named
        );
        assert!(
            ExperimentConfig::load(&named)
                .unwrap_err()
                .to_string()
                .contains("cannot read experiment manifest")
        );
    }

    #[test]
    fn timestamps_are_shaped() {
        assert_eq!(timestamp_compact().len(), 15);
        assert!(timestamp_rfc3339().ends_with('Z'));
    }
}
