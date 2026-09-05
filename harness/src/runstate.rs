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

/// `Manifest::selfcheck`: the D13 gate selfcheck was not requested (`--selfcheck` is opt-in).
pub const SELFCHECK_NOT_RUN: &str = "not-run";
/// `Manifest::selfcheck`: `SELFCHECK RESULT PASS`.
pub const SELFCHECK_PASS: &str = "pass";
/// `Manifest::selfcheck`: `SELFCHECK RESULT FAIL` from real verdicts (dispatch refused).
pub const SELFCHECK_FAIL: &str = "fail";
/// `Manifest::selfcheck`: a focused command was not runnable (rc 126/127; dispatch refused).
pub const SELFCHECK_NOVERDICT: &str = "noverdict";

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
    /// The trusted base commit the workspace started from (pushed with the task's chain).
    pub base_sha: String,
    /// `origin/main` HEAD the fresh clone started from (empty for manifests written before this
    /// field existed; it is the trusted commit's parent unless the overlay was a no-op).
    #[serde(default)]
    pub clone_sha: String,
    /// For a lifecycle run, the exact promoted predecessor that origin/main had to name before
    /// dispatch.  Empty for standalone runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle_predecessor_sha: Option<String>,
    pub session_id: String,
    /// herdr pane id of the workspace root pane.
    pub pane: String,
    /// herdr agent target name (renamed to `task` right after launch).
    #[serde(default = "default_agent_name")]
    pub agent_name: String,
    pub start: String,
    /// D13 gate selfcheck at dispatch: `not-run` | `pass` | `fail` | `noverdict`. Refusal
    /// (`fail` / `noverdict`) aborts before this manifest exists, so a dispatched run carries
    /// `not-run` or `pass`; the other two are the vocabulary of `runs/<ID>/selfcheck.log`.
    #[serde(default = "default_selfcheck")]
    pub selfcheck: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experiment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<GateRecord>,
    /// Terminal run state at gate time (`GOAL_MET`, `IDLE`, `AGENT_EXITED`, `KILLED_TIMEOUT`, ...) —
    /// the other half of the promotion decision, recorded beside the gate verdict. Empty for
    /// manifests written before this field existed.
    #[serde(default)]
    pub status_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_sha: Option<String>,
    /// Commit created from the immutable tree and durably recorded before the external push.
    /// This makes a crash between push and final record recoverable without recreating a commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_promotion_sha: Option<String>,
}

fn default_agent_name() -> String {
    "task".to_string()
}

fn default_selfcheck() -> String {
    SELFCHECK_NOT_RUN.to_string()
}

/// The gate verdict for one run, recorded by `taskfmt gate`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateRecord {
    /// Immutable gate-record format. Older records are deliberately non-promotable.
    #[serde(default)]
    pub schema: String,
    /// "pass" only when verify exited 0 with last line DONE.
    pub verdict: String,
    pub exit: i32,
    pub last_line: String,
    /// Workspace HEAD at gate time — what `promote` compares against.
    pub head: String,
    /// Complete staged candidate tree verified by this record.
    #[serde(default)]
    pub candidate_tree: String,
    /// Exact commit parent recorded before verification.
    #[serde(default)]
    pub parent: String,
    #[serde(default)]
    pub task_sha256: String,
    #[serde(default)]
    pub verifier_sha256: String,
    #[serde(default)]
    pub harness_fingerprint: String,
    #[serde(default)]
    pub evidence_sha256: String,
    /// Digest and location of canonical per-check matcher evidence.
    #[serde(default)]
    pub matcher_evidence_sha256: String,
    #[serde(default)]
    pub matcher_evidence: String,
    /// Terminal executor state that authorized this gate.
    #[serde(default)]
    pub terminal_state: String,
    #[serde(default)]
    pub started: String,
    pub log: String,
    pub finished: String,
}

impl Default for GateRecord {
    fn default() -> Self {
        Self {
            schema: String::new(),
            verdict: String::new(),
            exit: 1,
            last_line: String::new(),
            head: String::new(),
            candidate_tree: String::new(),
            parent: String::new(),
            task_sha256: String::new(),
            verifier_sha256: String::new(),
            harness_fingerprint: String::new(),
            evidence_sha256: String::new(),
            matcher_evidence_sha256: String::new(),
            matcher_evidence: String::new(),
            terminal_state: String::new(),
            started: String::new(),
            log: String::new(),
            finished: String::new(),
        }
    }
}

impl GateRecord {
    pub fn passed(&self) -> bool {
        self.verdict == "pass"
    }

    /// A record is promotion evidence only when all identities were written by the immutable
    /// gate. Deserialized v1 records intentionally fail this predicate.
    pub fn promotable(&self) -> bool {
        self.schema == "gate/v3"
            && self.passed()
            && !self.candidate_tree.is_empty()
            && !self.parent.is_empty()
            && !self.task_sha256.is_empty()
            && !self.verifier_sha256.is_empty()
            && !self.harness_fingerprint.is_empty()
            && !self.evidence_sha256.is_empty()
            && !self.matcher_evidence_sha256.is_empty()
            && !self.matcher_evidence.is_empty()
            && matches!(
                self.terminal_state.as_str(),
                "GOAL_MET" | "IDLE" | "GOAL_CLEARED_ERROR"
            )
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
    /// Tree of `result_sha`, recorded after promotion rather than inferred from a mutable ref.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_tree: Option<String>,
    /// Remote `main` observed after promotion. It must name `result_sha` for a completed task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_main_sha: Option<String>,
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

    /// Record one task's outcome. A re-run REPLACES the entry carrying the same task id instead of
    /// stacking a second one, so duplicate suppression cannot depend on statement order at any
    /// call site.
    pub fn record_task(&mut self, entry: ExperimentTask) {
        self.tasks.retain(|done| done.task != entry.task);
        self.tasks.push(entry);
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
            clone_sha: "parent0".into(),
            lifecycle_predecessor_sha: None,
            session_id: "00000000-0000-4000-8000-000000000000".into(),
            pane: "pane-1".into(),
            agent_name: default_agent_name(),
            start: "2026-08-28T10:10:10Z".into(),
            selfcheck: SELFCHECK_PASS.into(),
            experiment: Some("EXP-1".into()),
            gate: Some(GateRecord {
                verdict: "fail".into(),
                exit: 1,
                last_line: "RESULT FAIL".into(),
                head: "def456".into(),
                log: "/tmp/run/out/gate.log".into(),
                finished: "2026-08-28T11:00:00Z".into(),
                ..GateRecord::default()
            }),
            status_state: String::new(),
            result_sha: None,
            pending_promotion_sha: None,
        };
        manifest.save(dir.path()).unwrap();
        let loaded = Manifest::load(dir.path()).unwrap();
        assert_eq!(loaded.task, "TASK-101");
        assert!(!loaded.gate.as_ref().unwrap().passed());
        assert_eq!(loaded.agent_name, "task");
        assert_eq!(loaded.selfcheck, "pass");
    }

    #[test]
    fn manifest_without_selfcheck_field_reads_as_not_run() {
        let dir = tempfile::tempdir().unwrap();
        let json = r#"{"run":"r","run_dir":"/tmp/run","container":"c","agent":"p","agent_kind":"claude","model":"m","effort":"low","task":"TASK-001","repo_url":"u","base_sha":"abc","session_id":"sid","pane":"","start":""}"#;
        std::fs::write(dir.path().join(MANIFEST_FILE), json).unwrap();
        let loaded = Manifest::load(dir.path()).unwrap();
        assert_eq!(loaded.selfcheck, SELFCHECK_NOT_RUN);
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
                result_tree: None,
                remote_main_sha: None,
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

    /// The terminal run state survives a save-then-load round trip, and a manifest written before
    /// the field existed still loads with it empty. The JSON is printed collapsed onto one line
    /// because `save` goes through `to_vec_pretty`: the key and its value land on their own line,
    /// and a line-oriented reader of this output would otherwise never see them together.
    #[test]
    fn status_state_round_trips() {
        let dir = tempfile::tempdir().unwrap();

        // a manifest that OMITS the key loads with the field empty
        let legacy = r#"{"run":"r","run_dir":"/tmp/run","container":"c","agent":"p","agent_kind":"claude","model":"m","effort":"low","task":"TASK-001","repo_url":"u","base_sha":"abc","session_id":"sid","pane":"","start":""}"#;
        std::fs::write(dir.path().join(MANIFEST_FILE), legacy).unwrap();
        assert_eq!(Manifest::load(dir.path()).unwrap().status_state, "");

        let mut manifest = Manifest {
            run: "20260828-101010-claude-TASK-101".into(),
            run_dir: dir.path().display().to_string(),
            container: "harness-20260828-101010-claude-TASK-101".into(),
            agent: "zai-flash".into(),
            agent_kind: "claude".into(),
            model: "glm-5.3-flash".into(),
            effort: "low".into(),
            task: "TASK-101".into(),
            repo_url: "https://github.com/donbeave/x.git".into(),
            base_sha: "abc123".into(),
            clone_sha: "parent0".into(),
            lifecycle_predecessor_sha: None,
            session_id: "00000000-0000-4000-8000-000000000000".into(),
            pane: "pane-1".into(),
            agent_name: default_agent_name(),
            start: "2026-08-28T10:10:10Z".into(),
            selfcheck: SELFCHECK_PASS.into(),
            experiment: Some("EXP-1".into()),
            gate: None,
            status_state: String::new(),
            result_sha: None,
            pending_promotion_sha: None,
        };
        manifest.status_state = "GOAL_MET".into();
        manifest.save(dir.path()).unwrap();

        let text = std::fs::read_to_string(dir.path().join(MANIFEST_FILE)).unwrap();
        println!(
            "ROUNDTRIP-JSON {}",
            text.split_whitespace().collect::<Vec<_>>().join(" ")
        );
        let loaded = Manifest::load(dir.path()).unwrap();
        println!("ROUNDTRIP-LOADED status_state={}", loaded.status_state);
        assert_eq!(loaded.status_state, "GOAL_MET");
    }

    /// Duplicate suppression is a property of the recording function, not of the order in which a
    /// caller happens to write its statements: recording the same task id twice leaves one entry,
    /// the later one.
    #[test]
    fn record_task_replaces_the_earlier_entry() {
        let mut state = ExperimentState::new("EXP-1", "https://example.invalid/x.git");
        for (gate, pushed) in [("fail", false), ("pass", true)] {
            state.record_task(ExperimentTask {
                task: "TASK-001".into(),
                repo_url: state.repo_url.clone(),
                base_sha: "a".into(),
                result_sha: None,
                result_tree: None,
                remote_main_sha: None,
                gate: gate.into(),
                pushed,
                run_dir: "run".into(),
            });
        }
        println!(
            "RECORD-TASK entries={} gate={}",
            state.tasks.len(),
            state.tasks[0].gate
        );
        assert_eq!(state.tasks.len(), 1);
        assert_eq!(state.tasks[0].gate, "pass");
        assert!(state.tasks[0].pushed);
    }
}
