//! Immutable, non-lifecycle format studies (`study/v1`).
//!
//! A study is deliberately not an experiment: it has no run manifest, no remote state and no
//! promotion import.  It repeatedly gates detached copies of one recorded candidate tree.

use std::cmp::Reverse;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::gate::{self, GateOpts};
use crate::ops::git;
use crate::selfhost::hash::digest_file;
use crate::verifycfg::VerifyConfig;

pub const SCHEMA: &str = "study/v1";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudyConfig {
    pub schema: String,
    pub case_id: String,
    pub invariant_outcome: String,
    pub package_id: String,
    pub base_tree: String,
    pub verifier: VerifierIdentity,
    pub variants: Vec<Variant>,
    pub repeats: u32,
    pub blocks: u32,
    pub random_seed: u64,
    pub primary_endpoint: String,
    #[serde(default)]
    pub exclusions: Vec<String>,
    pub artifact_policy: String,
    pub image: String,
    pub agent: String,
    pub model: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifierIdentity {
    pub task_id: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    pub id: String,
    pub claim: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub variant: String,
    pub repeat: u32,
    pub block: u32,
    pub order: usize,
}

#[derive(Debug, Serialize)]
pub struct Observation {
    pub schema: &'static str,
    pub observation_id: String,
    pub assignment: AssignmentRecord,
    pub package_id: String,
    pub base_tree: String,
    pub candidate_tree: String,
    pub verifier: VerifierIdentityRecord,
    pub harness: String,
    pub image: String,
    pub agent: String,
    pub model: String,
    pub normalized_claim: String,
    pub gate: GateRecord,
    pub duration_ms: u128,
    pub retries: u32,
    pub rework: u32,
    pub scope_violations: u32,
    pub diff: DiffMetrics,
    pub artifact_policy: String,
}

#[derive(Debug, Serialize)]
pub struct AssignmentRecord {
    pub variant: String,
    pub repeat: u32,
    pub block: u32,
    pub order: usize,
}
#[derive(Debug, Serialize)]
pub struct VerifierIdentityRecord {
    pub task_id: String,
    pub sha256: String,
}
#[derive(Debug, Serialize)]
pub struct GateRecord {
    pub result: String,
    pub exit: i32,
    pub failed_checks: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct DiffMetrics {
    pub files: u32,
    pub additions: u32,
    pub deletions: u32,
}

impl StudyConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let cfg: Self = toml::from_str(
            &std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?,
        )?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.schema == SCHEMA,
            "study schema is {:?}, want {SCHEMA:?}",
            self.schema
        );
        for (name, value) in [
            ("case_id", &self.case_id),
            ("invariant_outcome", &self.invariant_outcome),
            ("package_id", &self.package_id),
            ("primary_endpoint", &self.primary_endpoint),
            ("artifact_policy", &self.artifact_policy),
            ("image", &self.image),
            ("agent", &self.agent),
            ("model", &self.model),
            ("verifier.task_id", &self.verifier.task_id),
            ("verifier.sha256", &self.verifier.sha256),
        ] {
            ensure!(!value.trim().is_empty(), "{name} empty");
        }
        ensure!(oid(&self.base_tree), "base_tree invalid");
        ensure!(sha256(&self.verifier.sha256), "verifier.sha256 invalid");
        ensure!(
            self.repeats > 0 && self.blocks > 0,
            "repeats and blocks must be positive"
        );
        ensure!(!self.variants.is_empty(), "variants empty");
        let mut ids = std::collections::BTreeSet::new();
        for variant in &self.variants {
            ensure!(
                !variant.id.trim().is_empty() && ids.insert(&variant.id),
                "variant id empty or duplicate: {}",
                variant.id
            );
            ensure!(
                !variant.claim.trim().is_empty(),
                "variant {} claim empty",
                variant.id
            );
        }
        Ok(())
    }
}

/// Balanced factorial cells in a seeded, deterministic order.  Every variant occurs exactly once
/// per `(block, repeat)` pair; only their execution order varies.
pub fn assignments(cfg: &StudyConfig) -> Vec<Assignment> {
    let mut cells: Vec<_> = (0..cfg.blocks)
        .flat_map(|block| {
            (0..cfg.repeats).flat_map(move |repeat| {
                cfg.variants.iter().map(move |variant| {
                    (
                        rank(cfg.random_seed, block, repeat, &variant.id),
                        block,
                        repeat,
                        variant.id.clone(),
                    )
                })
            })
        })
        .collect();
    cells.sort_by_key(|(rank, block, repeat, id)| (Reverse(*rank), id.clone(), *block, *repeat));
    cells
        .into_iter()
        .enumerate()
        .map(|(order, (_, block, repeat, variant))| Assignment {
            variant,
            repeat,
            block,
            order,
        })
        .collect()
}

/// Execute each planned observation.  A failure is data, never a loop terminator.
pub fn run(config: &Path, root: &Path, task_dir: &Path, out: &Path) -> anyhow::Result<i32> {
    let cfg = StudyConfig::load(config)?;
    let verify_path = task_dir.join(crate::verifycfg::FILE_NAME);
    let verifier = VerifyConfig::load(&verify_path)?;
    verify_invariants(&cfg, &verifier, &digest_file(&verify_path)?)?;
    ensure!(
        git::is_repo(root),
        "study root is not a git repository: {}",
        root.display()
    );
    let parent = git::head(root)?;
    // Studies inspect a committed immutable candidate only.  Unlike lifecycle gating, they must
    // not stage an executor's mutable workspace or alter its index.
    let candidate_tree = git::rev_parse(root, "HEAD^{tree}")?;
    let diff = diff_metrics(root, &cfg.base_tree)?;
    let mut lines = String::new();
    let mut failures = 0;
    for assignment in assignments(&cfg) {
        let variant = cfg
            .variants
            .iter()
            .find(|v| v.id == assignment.variant)
            .expect("assignment variant exists");
        let frozen = git::detached_tree_worktree(root, &candidate_tree, &parent)?;
        let started = Instant::now();
        let output = gate::run(GateOpts {
            root: frozen.path().to_path_buf(),
            task_dir: task_dir.to_path_buf(),
            progress: None,
            base: Some(cfg.base_tree.clone()),
            log_dir: None,
            fail_fast: false,
            enforce_task_contract: true,
        });
        let passed = output.is_pass();
        failures += (!passed) as u32;
        let scope_violations = output.check("scope").is_some_and(|ok| !ok) as u32;
        let record = Observation {
            schema: "study-observation/v1",
            observation_id: format!("{}-{:04}", cfg.case_id, assignment.order + 1),
            assignment: AssignmentRecord {
                variant: assignment.variant,
                repeat: assignment.repeat,
                block: assignment.block,
                order: assignment.order,
            },
            package_id: cfg.package_id.clone(),
            base_tree: cfg.base_tree.clone(),
            candidate_tree: candidate_tree.clone(),
            verifier: VerifierIdentityRecord {
                task_id: cfg.verifier.task_id.clone(),
                sha256: cfg.verifier.sha256.clone(),
            },
            harness: crate::HARNESS_FINGERPRINT.to_string(),
            image: cfg.image.clone(),
            agent: cfg.agent.clone(),
            model: cfg.model.clone(),
            normalized_claim: normalize_claim(&variant.claim),
            gate: GateRecord {
                result: if passed { "pass" } else { "fail" }.to_string(),
                exit: output.exit,
                failed_checks: output.failed_checks,
            },
            duration_ms: started.elapsed().as_millis(),
            retries: 0,
            rework: 0,
            scope_violations,
            diff: diff.clone(),
            artifact_policy: cfg.artifact_policy.clone(),
        };
        lines.push_str(&serde_json::to_string(&record)?);
        lines.push('\n');
    }
    crate::redact::write_scrubbed(out, lines.as_bytes())
        .with_context(|| format!("writing {}", out.display()))?;
    println!(
        "STUDY {} observations={} failures={} output={}",
        if failures == 0 { "PASS" } else { "FAIL" },
        assignments(&cfg).len(),
        failures,
        out.display()
    );
    Ok(if failures == 0 { 0 } else { 1 })
}

fn normalize_claim(claim: &str) -> String {
    claim.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn verify_invariants(
    cfg: &StudyConfig,
    verifier: &VerifyConfig,
    verifier_sha256: &str,
) -> anyhow::Result<()> {
    ensure!(
        verifier.task_id == cfg.verifier.task_id && verifier.task_id == cfg.package_id,
        "study package/verifier task identity mismatch"
    );
    ensure!(
        verifier.base_tree.as_deref() == Some(cfg.base_tree.as_str()),
        "study and verifier base_tree differ"
    );
    ensure!(
        verifier_sha256 == cfg.verifier.sha256,
        "study verifier sha256 differs from verify.toml"
    );
    Ok(())
}
fn oid(value: &str) -> bool {
    (value.len() == 40 || value.len() == 64) && value.chars().all(|c| c.is_ascii_hexdigit())
}
fn sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
fn rank(seed: u64, block: u32, repeat: u32, id: &str) -> u64 {
    id.bytes()
        .fold(seed ^ ((block as u64) << 32) ^ repeat as u64, |h, byte| {
            h.wrapping_mul(0x100000001b3).wrapping_add(byte as u64)
        })
}

fn diff_metrics(root: &Path, base: &str) -> anyhow::Result<DiffMetrics> {
    let output = git::output(std::process::Command::new("git").current_dir(root).args([
        "diff",
        "--numstat",
        base,
    ]))?;
    let mut result = DiffMetrics {
        files: 0,
        additions: 0,
        deletions: 0,
    };
    for row in output.lines() {
        let mut fields = row.splitn(3, '\t');
        let (Some(add), Some(delete), Some(_path)) = (fields.next(), fields.next(), fields.next())
        else {
            bail!("invalid git numstat row");
        };
        result.files += 1;
        result.additions += add.parse::<u32>().unwrap_or(0);
        result.deletions += delete.parse::<u32>().unwrap_or(0);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cfg() -> StudyConfig {
        StudyConfig {
            schema: SCHEMA.into(),
            case_id: "case".into(),
            invariant_outcome: "same gate".into(),
            package_id: "TASK-001".into(),
            base_tree: "0".repeat(40),
            verifier: VerifierIdentity {
                task_id: "TASK-001".into(),
                sha256: "a".repeat(64),
            },
            variants: vec![
                Variant {
                    id: "a".into(),
                    claim: "A".into(),
                },
                Variant {
                    id: "b".into(),
                    claim: "B".into(),
                },
            ],
            repeats: 2,
            blocks: 3,
            random_seed: 7,
            primary_endpoint: "gate".into(),
            exclusions: vec![],
            artifact_policy: "keep records".into(),
            image: "none".into(),
            agent: "test".into(),
            model: "test".into(),
        }
    }
    #[test]
    fn assignment_is_deterministic_and_balanced() {
        let c = cfg();
        let first = assignments(&c);
        assert_eq!(first, assignments(&c));
        assert_eq!(first.len(), 12);
        for variant in ["a", "b"] {
            assert_eq!(first.iter().filter(|x| x.variant == variant).count(), 6);
        }
    }
    #[test]
    fn invalid_identity_and_empty_study_refuse() {
        let mut c = cfg();
        c.verifier.sha256 = "x".into();
        assert!(c.validate().is_err());
        c = cfg();
        c.variants.clear();
        assert!(c.validate().is_err());
    }
    #[test]
    fn claim_normalization_is_stable() {
        assert_eq!(normalize_claim("  same\n claim  "), "same claim");
    }

    #[test]
    fn same_base_and_verifier_are_required() {
        let c = cfg();
        let mut verifier = VerifyConfig::parse(&format!(
            "schema = \"verify/v2\"\ntask_id = \"TASK-001\"\nbase_tree = \"{}\"\nwritable_paths = [\"src\"]\n[[checks]]\nid = \"CHK-001\"\nphase = \"gate\"\nargv = [\"true\"]\n",
            c.base_tree
        ))
        .unwrap();
        assert!(verify_invariants(&c, &verifier, &c.verifier.sha256).is_ok());
        verifier.base_tree = Some("1".repeat(40));
        assert!(verify_invariants(&c, &verifier, &c.verifier.sha256).is_err());
        verifier.base_tree = Some(c.base_tree.clone());
        assert!(verify_invariants(&c, &verifier, "badhash").is_err());
    }

    #[test]
    fn failure_record_fields_are_not_optional() {
        let value = serde_json::json!({
            "schema": "study-observation/v1", "gate": {"result": "fail"},
            "retries": 0, "rework": 0, "scope_violations": 1,
            "diff": {"files": 1, "additions": 2, "deletions": 3}
        });
        for field in [
            "schema",
            "gate",
            "retries",
            "rework",
            "scope_violations",
            "diff",
        ] {
            assert!(value.get(field).is_some(), "missing {field}");
        }
        assert_eq!(value["gate"]["result"], "fail");
    }

    #[test]
    fn failed_observations_are_all_written() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repo");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/ok.rs"), "// fixed candidate\n").unwrap();
        git::init(&root).unwrap();
        git::add_all_including_ignored(&root).unwrap();
        git::commit(&root, "baseline", false, false).unwrap();
        let base_tree = git::rev_parse(&root, "HEAD^{tree}").unwrap();

        let task = temp.path().join("task");
        std::fs::create_dir_all(&task).unwrap();
        std::fs::write(
            task.join("README.md"),
            include_str!("../testdata/example/README.md"),
        )
        .unwrap();
        let verify = include_str!("../testdata/example/verify.toml")
            .replace(
                "task_id = \"TASK-042\"",
                &format!("task_id = \"TASK-042\"\nbase_tree = \"{base_tree}\""),
            )
            .replace(
                "argv = [\"cargo\", \"test\", \"-p\", \"auth\", \"expired_refresh_token\"]",
                "argv = [\"false\"]",
            )
            .replace(
                "argv = [\"cargo\", \"test\", \"-p\", \"auth\", \"valid_refresh_rotation\"]",
                "argv = [\"true\"]",
            )
            .replace(
                "shell = \"! grep -rn legacy_expiry_check src tests\"",
                "argv = [\"true\"]",
            )
            .replace("argv = [\"taskfmt\", \"verify\"]", "argv = [\"true\"]");
        let verifier_path = task.join("verify.toml");
        std::fs::write(&verifier_path, verify).unwrap();
        let study_path = temp.path().join("study.toml");
        std::fs::write(&study_path, format!(
            "schema = \"study/v1\"\ncase_id = \"failure-capture\"\ninvariant_outcome = \"same verifier\"\npackage_id = \"TASK-042\"\nbase_tree = \"{base_tree}\"\nrepeats = 2\nblocks = 1\nrandom_seed = 1\nprimary_endpoint = \"gate\"\nartifact_policy = \"ndjson\"\nimage = \"none\"\nagent = \"test\"\nmodel = \"test\"\n[verifier]\ntask_id = \"TASK-042\"\nsha256 = \"{}\"\n[[variants]]\nid = \"a\"\nclaim = \"first\"\n[[variants]]\nid = \"b\"\nclaim = \"second\"\n", digest_file(&verifier_path).unwrap())).unwrap();
        let out = temp.path().join("observations.ndjson");
        assert_eq!(run(&study_path, &root, &task, &out).unwrap(), 1);
        let records: Vec<serde_json::Value> = std::fs::read_to_string(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(
            records.len(),
            4,
            "a failed gate cannot censor later assignments"
        );
        assert!(
            records
                .iter()
                .all(|record| record["gate"]["result"] == "fail")
        );
    }
}
