//! `taskfmt ps` — the run containers on this host, asked of docker rather than of a manifest.
//!
//! Every other run command needs an `experiment.toml` before it can say what a run is. This one
//! needs nothing but the docker daemon, because a run container carries its own identity: the
//! `taskfmt.*` labels it was launched with, and — for containers launched before those labels
//! existed — the `/work` bind mount whose parent is the run directory. That is what makes it the
//! command an operator can always run first, from anywhere, and the one every "cannot locate a run"
//! error points at.
//!
//! It is read-only and never prompts: it inspects containers and reads `manifest.json` when there
//! is one, and writes nothing.

use anyhow::bail;
use serde::Serialize;

use crate::ops::container::{
    self, CONTAINER_PREFIX, LABEL_EXP, LABEL_MANIFEST, LABEL_PROFILE, LABEL_TASK,
};
use crate::ops::docker::{self, ContainerInfo};
use crate::redact;
use crate::runstate::Manifest;

/// One listed run. Every field is either the container's own statement about itself or a value read
/// from the run's `manifest.json`; unknown fields are empty rather than guessed.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub run: String,
    pub container: String,
    pub task: String,
    pub profile: String,
    /// The container's docker state: `running`, `exited`, …
    pub state: String,
    /// `manifest.json`'s `status_state` — the terminal state `gate` recorded, when it ran.
    pub status: String,
    /// `out/status.json`'s `terminal_reason` — WHY the harness decided the run had ended
    /// (`goal-verdict`, `agent-exited`, `container-stopped`, `killed-timeout`,
    /// `goal-cleared-error`). A state name alone does not say which signal produced it, and that
    /// is the question an operator asks first when a run ends earlier than it should have.
    pub reason: String,
    /// The recorded gate verdict, when the gate has run.
    pub gate: String,
    pub run_dir: String,
    /// The `experiment.toml` the run was dispatched with, when the container names it.
    pub manifest: String,
    /// The experiment id, when the run belongs to one.
    pub exp: String,
}

impl Row {
    /// Describe one container. The manifest is a bonus, not a requirement: a run whose directory
    /// has been moved or deleted still lists, with the fields docker knows.
    pub fn from_container(info: &ContainerInfo) -> Self {
        let run_dir = container::run_dir_of(info);
        let manifest = run_dir.as_deref().and_then(|dir| Manifest::load(dir).ok());
        let label = |key: &str| info.label(key).unwrap_or_default().to_string();
        Self {
            run: container::run_id_of(info),
            container: info.name.clone(),
            task: manifest
                .as_ref()
                .map(|m| m.task.clone())
                .unwrap_or_else(|| label(LABEL_TASK)),
            profile: manifest
                .as_ref()
                .map(|m| m.agent.clone())
                .unwrap_or_else(|| label(LABEL_PROFILE)),
            state: info.state.clone(),
            status: manifest
                .as_ref()
                .map(|m| m.status_state.clone())
                .unwrap_or_default(),
            reason: run_dir
                .as_deref()
                .and_then(terminal_reason_of)
                .unwrap_or_default(),
            gate: manifest
                .as_ref()
                .and_then(|m| m.gate.as_ref().map(|gate| gate.verdict.clone()))
                .unwrap_or_default(),
            run_dir: run_dir
                .map(|dir| dir.display().to_string())
                .unwrap_or_default(),
            manifest: container::manifest_of(info)
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| label(LABEL_MANIFEST)),
            exp: manifest
                .as_ref()
                .and_then(|m| m.experiment.clone())
                .unwrap_or_else(|| label(LABEL_EXP)),
        }
    }
}

/// `terminal_reason` from the run's recorded terminal status, when `gate` wrote one.
fn terminal_reason_of(run_dir: &std::path::Path) -> Option<String> {
    let path = run_dir.join("out").join(crate::cmds::run::STATUS_FILE);
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    value
        .get("terminal_reason")?
        .as_str()
        .map(std::string::ToString::to_string)
}

/// One table column: its header, and the field of a row it prints.
type Column = (&'static str, fn(&Row) -> &str);

/// The table's columns, in order.
const COLUMNS: [Column; 8] = [
    ("RUN", |row| &row.run),
    ("TASK", |row| &row.task),
    ("PROFILE", |row| &row.profile),
    ("STATE", |row| &row.state),
    ("STATUS", |row| &row.status),
    ("REASON", |row| &row.reason),
    ("GATE", |row| &row.gate),
    ("RUN DIR", |row| &row.run_dir),
];

/// Placeholder for a field nothing on this host can answer.
const UNKNOWN: &str = "-";

fn cell(value: &str) -> &str {
    if value.is_empty() { UNKNOWN } else { value }
}

/// Every `harness-*` container, newest first. Run ids start with `YYYYMMDD-HHMMSS`, so
/// reverse-lexicographic by run id is newest first — the same order `cmds::recent_runs` uses.
pub fn rows() -> Vec<Row> {
    let names = docker::list_containers(CONTAINER_PREFIX);
    let mut rows: Vec<Row> = docker::inspect_containers(&names)
        .iter()
        .map(Row::from_container)
        .collect();
    rows.sort_by(|a, b| b.run.cmp(&a.run));
    rows
}

/// Render the table: one header line, one line per run, columns padded to their widest cell.
pub fn table(rows: &[Row]) -> Vec<String> {
    let widths: Vec<usize> = COLUMNS
        .iter()
        .map(|(header, field)| {
            rows.iter()
                .map(|row| cell(field(row)).chars().count())
                .chain(std::iter::once(header.chars().count()))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let line = |cells: Vec<&str>| {
        let mut out = String::new();
        for (index, text) in cells.iter().enumerate() {
            if index + 1 == cells.len() {
                out.push_str(text);
            } else {
                out.push_str(&format!("{:width$}  ", text, width = widths[index]));
            }
        }
        out
    };
    let mut lines = vec![line(COLUMNS.iter().map(|(header, _)| *header).collect())];
    for row in rows {
        lines.push(line(
            COLUMNS.iter().map(|(_, field)| cell(field(row))).collect(),
        ));
    }
    lines
}

pub fn run(json: bool) -> anyhow::Result<i32> {
    if !docker::available() {
        bail!(
            "cannot reach the docker daemon (`docker version` failed) — the run containers are \
             what `taskfmt ps` lists, so there is nothing it can answer without it"
        );
    }
    let rows = rows();
    if rows.is_empty() {
        redact::eemit(&format!(
            "no {CONTAINER_PREFIX}* containers on this host (`taskfmt run --task <TASK>` \
             dispatches one)"
        ));
        return Ok(0);
    }
    if json {
        for row in &rows {
            redact::emit(&serde_json::to_string(row)?);
        }
    } else {
        redact::emit_lines(table(&rows));
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn labelled(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn a_labelled_container_lists_without_any_manifest_on_disk() {
        let info = ContainerInfo {
            name: "harness-20260101-000000-zai-flash-TASK-001".into(),
            state: "running".into(),
            work_mount: None,
            labels: labelled(&[
                (
                    container::LABEL_RUN_ID,
                    "20260101-000000-zai-flash-TASK-001",
                ),
                (container::LABEL_RUN_DIR, "/runs/20260101-000000"),
                (LABEL_TASK, "TASK-001"),
                (LABEL_PROFILE, "zai-flash"),
                (LABEL_EXP, "exp-20260101-000000"),
            ]),
        };
        let row = Row::from_container(&info);
        assert_eq!(row.run, "20260101-000000-zai-flash-TASK-001");
        assert_eq!(row.task, "TASK-001");
        assert_eq!(row.profile, "zai-flash");
        assert_eq!(row.state, "running");
        assert_eq!(row.run_dir, "/runs/20260101-000000");
        assert_eq!(row.exp, "exp-20260101-000000");
        // nothing to read a verdict from, so neither is guessed
        assert_eq!(row.status, "");
        assert_eq!(row.gate, "");
    }

    #[test]
    fn a_pre_label_container_lists_from_its_work_mount_and_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path().join("20260829-194052-zai-flash-TASK-002");
        std::fs::create_dir_all(run_dir.join("workspace")).unwrap();
        let manifest = crate::runstate::Manifest {
            run: "20260829-194052-zai-flash-TASK-002".into(),
            run_dir: run_dir.display().to_string(),
            container: "harness-20260829-194052-zai-flash-TASK-002".into(),
            agent: "zai-flash".into(),
            agent_kind: "claude".into(),
            model: "m".into(),
            effort: "high".into(),
            task: "TASK-002".into(),
            repo_url: "u".into(),
            base_sha: "abc".into(),
            clone_sha: String::new(),
            session_id: "sid".into(),
            pane: String::new(),
            agent_name: "task".into(),
            start: String::new(),
            selfcheck: crate::runstate::SELFCHECK_NOT_RUN.into(),
            experiment: None,
            gate: Some(crate::runstate::GateRecord {
                verdict: "pass".into(),
                exit: 0,
                last_line: "DONE".into(),
                head: "head".into(),
                log: "/tmp/gate.log".into(),
                finished: "2026-08-29T20:00:00Z".into(),
            }),
            status_state: "GOAL_MET".into(),
            result_sha: None,
        };
        manifest.save(&run_dir).unwrap();
        let info = ContainerInfo {
            name: "harness-20260829-194052-zai-flash-TASK-002".into(),
            state: "running".into(),
            work_mount: Some(run_dir.join("workspace")),
            labels: BTreeMap::new(),
        };
        let row = Row::from_container(&info);
        assert_eq!(row.run, "20260829-194052-zai-flash-TASK-002");
        assert_eq!(row.task, "TASK-002", "read from manifest.json, not a label");
        assert_eq!(row.profile, "zai-flash");
        assert_eq!(row.status, "GOAL_MET");
        assert_eq!(
            row.reason, "",
            "no recorded terminal status yet, and none is invented"
        );
        assert_eq!(row.gate, "pass");
        // once `wait_and_gate` has recorded the terminal status, ps says WHY the run ended
        crate::redact::write_json(
            &run_dir.join("out").join(crate::cmds::run::STATUS_FILE),
            &serde_json::json!({"state": "GOAL_MET", "terminal_reason": "goal-verdict"}),
        )
        .unwrap();
        assert_eq!(Row::from_container(&info).reason, "goal-verdict");
        assert_eq!(row.run_dir, run_dir.display().to_string());
        assert_eq!(row.manifest, "", "no label, and none is invented");
    }

    #[test]
    fn a_container_with_nothing_readable_still_lists() {
        let info = ContainerInfo {
            name: "harness-ghost".into(),
            state: "exited".into(),
            ..Default::default()
        };
        let row = Row::from_container(&info);
        assert_eq!(row.run, "ghost");
        assert_eq!(row.state, "exited");
        assert_eq!(row.run_dir, "");
        let lines = table(&[row]);
        assert!(lines[0].starts_with("RUN "), "{:?}", lines[0]);
        // unknown cells print as `-`, never blank: a blank column reads like a rendering bug
        assert!(lines[1].contains(" -  "), "{:?}", lines[1]);
        assert!(lines[1].ends_with('-'), "{:?}", lines[1]);
    }

    #[test]
    fn the_table_pads_to_the_widest_cell_and_the_json_is_one_object_per_line() {
        let wide = ContainerInfo {
            name: "harness-20260101-000000-zai-flash-TASK-001".into(),
            state: "running".into(),
            labels: labelled(&[(LABEL_TASK, "TASK-001")]),
            ..Default::default()
        };
        let narrow = ContainerInfo {
            name: "harness-r".into(),
            state: "exited".into(),
            ..Default::default()
        };
        let rows = vec![Row::from_container(&wide), Row::from_container(&narrow)];
        let lines = table(&rows);
        assert_eq!(lines.len(), 3);
        let run_width = "20260101-000000-zai-flash-TASK-001".len();
        assert!(lines[2].starts_with(&format!("{:width$}  ", "r", width = run_width)));
        let json = serde_json::to_string(&rows[0]).unwrap();
        assert!(!json.contains('\n'), "one line per run: {json}");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["task"], "TASK-001");
        assert_eq!(
            parsed["container"],
            "harness-20260101-000000-zai-flash-TASK-001"
        );
    }
}
