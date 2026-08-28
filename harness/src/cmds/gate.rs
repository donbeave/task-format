//! `taskfmt gate <RUN>` — the host gate: re-run verify on the workspace with trusted copies and
//! record the verdict in the manifest.

use std::path::Path;

use anyhow::Context;

use crate::cmds::Ctx;
use crate::config::Resolved;
use crate::gate::{self, GateOpts};
use crate::redact;
use crate::runstate::{GateRecord, Manifest};

pub fn run(ctx: &Ctx, run_id: &str) -> anyhow::Result<i32> {
    let resolved = ctx.load()?;
    let run_dir = crate::cmds::resolve_run_arg(&resolved, run_id)?;
    let mut manifest = Manifest::load(&run_dir)?;
    let passed = gate_run(&run_dir, &resolved, &mut manifest)?;

    redact::emit(&format!(
        "GATE {} ({}) log={}",
        if passed { "PASS" } else { "FAIL" },
        run_id,
        manifest
            .gate
            .as_ref()
            .map(|gate| gate.log.clone())
            .unwrap_or_default()
    ));
    Ok(manifest
        .gate
        .as_ref()
        .map(|gate| gate.exit)
        .unwrap_or(1)
        .max(if passed { 0 } else { 1 }))
}

/// Gate one run: trusted copies, the recorded base commit, progress from the run dir.
pub fn gate_run(
    run_dir: &Path,
    _resolved: &Resolved,
    manifest: &mut Manifest,
) -> anyhow::Result<bool> {
    let workspace = run_dir.join("workspace");
    let snapshot = run_dir.join("task-snapshot");
    let progress_file = run_dir.join("progress/progress.md");
    let out_dir = run_dir.join("out");
    let base = if manifest.base_sha.is_empty() {
        "baseline".to_string()
    } else {
        manifest.base_sha.clone()
    };

    let output = gate::run(GateOpts {
        root: workspace.clone(),
        task_dir: snapshot,
        progress: Some(progress_file.display().to_string()),
        base: Some(base),
        log_dir: Some(out_dir.join("gate-logs")),
        fail_fast: false,
    });

    let gate_log = out_dir.join("gate.log");
    redact::write_scrubbed(&gate_log, output.text.as_bytes())
        .with_context(|| format!("writing {}", gate_log.display()))?;

    let head = crate::ops::git::head(&workspace).unwrap_or_default();
    manifest.gate = Some(GateRecord {
        verdict: if output.is_pass() {
            "pass".to_string()
        } else {
            "fail".to_string()
        },
        exit: output.exit,
        last_line: output.last_line.clone(),
        head,
        log: gate_log.display().to_string(),
        finished: crate::config::timestamp_rfc3339(),
    });
    manifest.save(run_dir)?;
    let passed = manifest.gate.as_ref().is_some_and(|gate| gate.passed());
    Ok(passed)
}
