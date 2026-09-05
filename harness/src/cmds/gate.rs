//! `taskfmt gate <RUN>` — the host gate: re-run verify on the workspace with trusted copies and
//! record the verdict in the manifest.

use std::path::Path;
#[cfg(test)]
use std::process::Command;

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
    let matcher_evidence = out_dir.join("gate-evidence.json");
    let matcher_evidence_sha256 = write_matcher_evidence(&matcher_evidence, &output)?;

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
        schema: "gate/v3".to_string(),
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
        matcher_evidence_sha256,
        matcher_evidence: matcher_evidence.display().to_string(),
        terminal_state: manifest.status_state.clone(),
        started,
        log: gate_log.display().to_string(),
        finished: crate::config::timestamp_rfc3339(),
    });
    manifest.save(run_dir)?;
    Ok(manifest.gate.as_ref().is_some_and(GateRecord::passed))
}

fn write_matcher_evidence(path: &Path, output: &gate::GateOutput) -> anyhow::Result<String> {
    let bytes = gate::evidence_json(&output.checks)?;
    redact::write_scrubbed(path, &bytes).with_context(|| format!("writing {}", path.display()))?;
    crate::selfhost::hash::digest_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn real_gate(root: &Path) -> gate::GateOutput {
        std::fs::create_dir(root.join("src")).unwrap();
        std::fs::write(
            root.join("verify.toml"),
            "schema = \"verify/v2\"\ntask_id = \"TASK-001\"\nbase_tree = \"0123456789012345678901234567890123456789\"\nwritable_paths = [\"src\"]\n[[checks]]\nid = \"CHK-001\"\nphase = \"gate\"\nshell = \"printf x >> invoked; printf stable-output\"\nexpected = { stdout_contains = [\"stable-output\"] }\n",
        )
        .unwrap();
        git(root, &["init", "-q"]);
        git(root, &["config", "user.name", "Matcher Test"]);
        git(root, &["config", "user.email", "matcher@test.invalid"]);
        git(root, &["add", "verify.toml"]);
        git(root, &["commit", "-qm", "base"]);
        let output = gate::run(gate::GateOpts {
            root: root.to_path_buf(),
            task_dir: root.to_path_buf(),
            progress: None,
            base: Some("HEAD".into()),
            log_dir: Some(root.join("logs")),
            fail_fast: false,
            enforce_task_contract: false,
        });
        assert!(output.is_pass(), "{}", output.text);
        assert_eq!(std::fs::read_to_string(root.join("invoked")).unwrap(), "x");
        output
    }

    #[test]
    fn matcher_evidence_is_canonical_and_digestable() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("gate-evidence.json");
        let output = gate::GateOutput {
            exit: 1,
            text: String::new(),
            last_line: "RESULT FAIL".into(),
            summary: String::new(),
            log_dir: Path::new("/irrelevant").into(),
            failed_checks: vec!["CHK-001".into()],
            checks: vec![gate::CheckResult {
                name: "CHK-001".into(),
                pass: false,
                rc: 1,
                evidence: Some(gate::CommandEvidence {
                    exit: 0,
                    stdout: "out".into(),
                    stderr: String::new(),
                    matchers: vec![gate::MatcherResult {
                        kind: "stdout.contains".into(),
                        expected: "wanted".into(),
                        actual: "false".into(),
                        pass: false,
                    }],
                }),
            }],
        };
        let first_digest = write_matcher_evidence(&path, &output).unwrap();
        let first = std::fs::read(&path).unwrap();
        let second_digest = write_matcher_evidence(&path, &output).unwrap();
        assert_eq!(first, std::fs::read(&path).unwrap());
        assert_eq!(first_digest, second_digest);
        assert_eq!(
            first_digest,
            crate::selfhost::hash::digest_file(&path).unwrap()
        );
        assert!(
            std::str::from_utf8(&first)
                .unwrap()
                .contains("gate-evidence/v1")
        );
    }

    #[test]
    fn real_gate_matcher_evidence_has_stable_digest_and_complete_content() {
        let first_root = tempfile::tempdir().unwrap();
        let second_root = tempfile::tempdir().unwrap();
        let first_output = real_gate(first_root.path());
        let second_output = real_gate(second_root.path());
        let first_path = first_root.path().join("gate-evidence.json");
        let second_path = second_root.path().join("gate-evidence.json");
        let first_digest = write_matcher_evidence(&first_path, &first_output).unwrap();
        let second_digest = write_matcher_evidence(&second_path, &second_output).unwrap();
        let first = std::fs::read(&first_path).unwrap();
        assert_eq!(first, std::fs::read(&second_path).unwrap());
        assert_eq!(first_digest, second_digest);
        assert_eq!(
            first_digest,
            crate::selfhost::hash::digest_file(&first_path).unwrap()
        );
        let value: serde_json::Value = serde_json::from_slice(&first).unwrap();
        assert_eq!(value["schema"], "gate-evidence/v1");
        let checks = value["checks"].as_array().unwrap();
        assert_eq!(
            checks
                .iter()
                .map(|check| check["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "config",
                "scope",
                "forbidden_paths",
                "forbidden_patterns",
                "CHK-001"
            ]
        );
        let evidence = &checks[4]["evidence"];
        assert_eq!(evidence["exit"], 0);
        assert_eq!(evidence["stdout"], "stable-output");
        assert_eq!(evidence["stderr"], "");
        assert_eq!(evidence["matchers"][0]["kind"], "exit");
        assert_eq!(evidence["matchers"][1]["kind"], "stdout.contains");
        assert_eq!(evidence["matchers"][0]["pass"], true);
        assert_eq!(evidence["matchers"][1]["pass"], true);
    }
}
