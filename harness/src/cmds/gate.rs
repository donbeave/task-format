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
    let (resolved, run_dir) = crate::cmds::load_run(ctx, run_id)?;
    let mut manifest = Manifest::load(&run_dir)?;
    let status = crate::cmds::status::check(&manifest, &run_dir)?;
    manifest.status_state = status.state.clone();
    // `gate` is also callable directly, not only through `run --wait`.  Remove the executor
    // writer before freezing the candidate in both paths.
    crate::cmds::run::quiesce(&manifest);
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
    let base = manifest.base_sha.clone();
    let started = crate::config::timestamp_rfc3339();

    // The index is our candidate snapshot.  It includes every worktree state, including ignored
    // and untracked paths, so promotion cannot silently drop work the agent produced.
    crate::ops::git::add_all_including_ignored(&workspace)?;
    let parent = crate::ops::git::head(&workspace)?;
    let candidate_tree = crate::ops::git::write_tree(&workspace)?;
    let task_sha256 = crate::selfhost::hash::digest_file(&snapshot.join("README.md"))?;
    let verifier_sha256 = crate::selfhost::hash::digest_file(&snapshot.join("verify.toml"))?;

    // Verify a detached materialization of the recorded tree, never the executor's mutable
    // workspace. The synthetic checkout retains the recorded parent so scope checks see the same
    // history a later promotion will use.
    let frozen = crate::ops::git::detached_tree_worktree(&workspace, &candidate_tree, &parent)?;

    let output = gate::run(GateOpts {
        root: frozen.path().to_path_buf(),
        task_dir: snapshot,
        progress: Some(progress_file.display().to_string()),
        base: Some(base),
        log_dir: Some(out_dir.join("gate-logs")),
        fail_fast: false,
        enforce_task_contract: true,
    });

    let gate_log = out_dir.join("gate.log");
    redact::write_scrubbed(&gate_log, output.text.as_bytes())
        .with_context(|| format!("writing {}", gate_log.display()))?;

    let head = crate::ops::git::head(&workspace)?;
    // Re-stage only to observe the complete post-verification tree.  A command which writes
    // after (or during) verification cannot change the tree later promoted.
    crate::ops::git::add_all_including_ignored(&workspace)?;
    let observed_tree = crate::ops::git::write_tree(&workspace)?;
    let immutable = observed_tree == candidate_tree;
    if !immutable {
        let detail = format!(
            "IMMUTABLE CANDIDATE ABORT expected_tree={candidate_tree} observed_tree={observed_tree}\n"
        );
        let mut prior = std::fs::read_to_string(&gate_log)
            .with_context(|| format!("reading {}", gate_log.display()))?;
        prior.push_str(&detail);
        redact::write_scrubbed(&gate_log, prior.as_bytes())
            .with_context(|| format!("writing {}", gate_log.display()))?;
    }
    let evidence_sha256 = crate::selfhost::hash::digest_file(&gate_log)?;
    manifest.gate = Some(GateRecord {
        schema: "gate/v2".to_string(),
        verdict: if immutable && output.is_pass() {
            "pass"
        } else if immutable {
            "fail"
        } else {
            "abort"
        }
        .to_string(),
        exit: if immutable { output.exit } else { 1 },
        last_line: if immutable {
            output.last_line
        } else {
            "RESULT FAIL".to_string()
        },
        head,
        candidate_tree,
        parent,
        task_sha256,
        verifier_sha256,
        harness_fingerprint: crate::HARNESS_FINGERPRINT.to_string(),
        evidence_sha256,
        terminal_state: manifest.status_state.clone(),
        started,
        log: gate_log.display().to_string(),
        finished: crate::config::timestamp_rfc3339(),
    });
    manifest.save(run_dir)?;
    Ok(manifest.gate.as_ref().is_some_and(GateRecord::passed))
}
