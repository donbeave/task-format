//! Per-run manifest (`manifest.json`) and experiment state (`experiment.json`).
//!
//! Both are artifacts the harness writes, so they go through the redactor on write. The manifest is
//! also the structural hand-off between commands: `gate` records the verdict here and `promote`
//! refuses to push unless that verdict says PASS — that is what makes "never push on gate FAIL"
//! structural rather than a matter of re-running the gate.

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::redact;

pub const MANIFEST_FILE: &str = "manifest.json";
pub const EXPERIMENT_FILE: &str = "experiment.json";

/// State of one dispatched run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub run: String,
    pub run_dir: String,
    pub container: String,
    /// Agent profile name from experiment.toml.
    pub agent: String,
    /// claude | codex.
    pub agent_kind: String,
    pub model: String,
    pub effort: String,
    pub task: String,
    pub repo_url: String,
    /// The trusted base commit the workspace started from (never pushed).
    pub base_sha: String,
    pub session_id: String,
    /// herdr pane id of the workspace root pane.
    pub pane: String,
    /// herdr agent target name (renamed to `task` right after launch).
    #[serde(default = "default_agent_name")]
    pub agent_name: String,
    pub start: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experiment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<GateRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_sha: Option<String>,
}

fn default_agent_name() -> String {
    "task".to_string()
}

/// The gate verdict for one run, recorded by `taskfmt gate`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateRecord {
    /// "pass" only when verify exited 0 with last line DONE.
    pub verdict: String,
    pub exit: i32,
    pub last_line: String,
    /// Workspace HEAD at gate time — what `promote` compares against.
    pub head: String,
    pub log: String,
    pub finished: String,
}

impl GateRecord {
    pub fn passed(&self) -> bool {
        self.verdict == "pass"
    }
}

impl Manifest {
    pub fn load(run_dir: &Path) -> anyhow::Result<Self> {
        let path = run_dir.join(MANIFEST_FILE);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let manifest: Manifest =
            serde_json::from_str(&text).with_context(|| format!("in {}", path.display()))?;
        Ok(manifest)
    }

    pub fn save(&self, run_dir: &Path) -> anyhow::Result<()> {
        redact::write_json(&run_dir.join(MANIFEST_FILE), self)?;
        Ok(())
    }

    pub fn run_id(&self) -> &str {
        &self.run
    }

    /// The run directory as a path.
    pub fn run_dir_path(&self) -> PathBuf {
        PathBuf::from(&self.run_dir)
    }

    /// `<run>/out`
    pub fn out_dir(&self) -> PathBuf {
        self.run_dir_path().join("out")
    }

    /// `<run>/out/tui.log` — the raw `script(1)` stream of the agent pane.
    pub fn tui_log(&self) -> PathBuf {
        self.out_dir().join("tui.log")
    }
}

/// One recorded task inside an experiment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentTask {
    pub task: String,
    pub repo_url: String,
    pub base_sha: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_sha: Option<String>,
    pub gate: String,
    pub pushed: bool,
    pub run_dir: String,
}

/// Experiment-level state, written after every task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentState {
    pub id: String,
    pub repo_url: String,
    pub started: String,
    pub tasks: Vec<ExperimentTask>,
}

impl ExperimentState {
    pub fn new(id: &str, repo_url: &str) -> Self {
        Self {
            id: id.to_string(),
            repo_url: repo_url.to_string(),
            started: crate::config::timestamp_rfc3339(),
            tasks: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> anyhow::Result<Option<Self>> {
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let state: ExperimentState =
            serde_json::from_str(&text).with_context(|| format!("in {}", path.display()))?;
        Ok(Some(state))
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        redact::write_json(path, self)?;
        Ok(())
    }

    /// Task ids already gated PASS (and pushed) — what `experiment --resume` skips.
    pub fn passed_tasks(&self) -> Vec<String> {
        self.tasks
            .iter()
            .filter(|task| task.gate == "pass" && task.pushed)
            .map(|task| task.task.clone())
            .collect()
    }

    pub fn experiment_dir(runs_dir: &Path, id: &str) -> PathBuf {
        runs_dir.join(id)
    }
}

/// A run directory: `runs_dir/<ts>-<profile>-<task>/`.
pub fn run_dir_name(timestamp: &str, profile: &str, task: &str) -> String {
    format!("{timestamp}-{profile}-{task}")
}

/// A repository created (or recorded) by the harness: `runs_dir/repos/<name>.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoRecord {
    pub name: String,
    pub url: String,
    pub created: String,
}

impl RepoRecord {
    fn dir(runs_dir: &Path) -> PathBuf {
        runs_dir.join("repos")
    }

    fn path(runs_dir: &Path, name: &str) -> PathBuf {
        Self::dir(runs_dir).join(format!("{name}.json"))
    }

    pub fn save(&self, runs_dir: &Path) -> anyhow::Result<()> {
        redact::write_json(&Self::path(runs_dir, &self.name), self)?;
        Ok(())
    }

    pub fn load_all(runs_dir: &Path) -> anyhow::Result<Vec<Self>> {
        let dir = Self::dir(runs_dir);
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        entries.sort();
        for path in entries {
            let text = std::fs::read_to_string(&path)?;
            out.push(
                serde_json::from_str(&text).with_context(|| format!("in {}", path.display()))?,
            );
        }
        Ok(out)
    }

    pub fn remove(runs_dir: &Path, name: &str) -> anyhow::Result<()> {
        let path = Self::path(runs_dir, name);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trips_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = Manifest {
            run: "20260828-101010-claude-TASK-101".into(),
            run_dir: "/tmp/run".into(),
            container: "harness-20260828-101010-claude-TASK-101".into(),
            agent: "zai-flash".into(),
            agent_kind: "claude".into(),
            model: "glm-5.3-flash".into(),
            effort: "low".into(),
            task: "TASK-101".into(),
            repo_url: "https://github.com/donbeave/x.git".into(),
            base_sha: "abc123".into(),
            session_id: "00000000-0000-4000-8000-000000000000".into(),
            pane: "pane-1".into(),
            agent_name: default_agent_name(),
            start: "2026-08-28T10:10:10Z".into(),
            experiment: Some("EXP-1".into()),
            gate: Some(GateRecord {
                verdict: "fail".into(),
                exit: 1,
                last_line: "RESULT FAIL".into(),
                head: "def456".into(),
                log: "/tmp/run/out/gate.log".into(),
                finished: "2026-08-28T11:00:00Z".into(),
            }),
            result_sha: None,
        };
        manifest.save(dir.path()).unwrap();
        let loaded = Manifest::load(dir.path()).unwrap();
        assert_eq!(loaded.task, "TASK-101");
        assert!(!loaded.gate.as_ref().unwrap().passed());
        assert_eq!(loaded.agent_name, "task");
    }

    #[test]
    fn experiment_state_tracks_passed_tasks_for_resume() {
        let mut state = ExperimentState::new("EXP-1", "https://example.invalid/x.git");
        for (task, gate, pushed) in [
            ("TASK-101", "pass", true),
            ("TASK-102", "fail", false),
            ("TASK-103", "pass", false),
        ] {
            state.tasks.push(ExperimentTask {
                task: task.into(),
                repo_url: state.repo_url.clone(),
                base_sha: "a".into(),
                result_sha: None,
                gate: gate.into(),
                pushed,
                run_dir: "run".into(),
            });
        }
        assert_eq!(state.passed_tasks(), vec!["TASK-101".to_string()]);
    }

    #[test]
    fn run_dir_name_shape() {
        assert_eq!(
            run_dir_name("20260828-142017", "claude", "TASK-101"),
            "20260828-142017-claude-TASK-101"
        );
    }
}
