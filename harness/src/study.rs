//! Immutable, non-lifecycle format studies (`study/v1`).
//!
//! A study is deliberately not an experiment: it has no run manifest, no remote state and no
//! promotion import.  It gates distinct material task-package overlays against fresh detached
//! copies of one recorded base tree.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
    /// Relative directory overlaid onto a fresh copy of the trusted task package for this
    /// observation.  It is deliberately material rather than a label: the gate receives this
    /// package, not the unmodified `--task-dir` package.
    pub overlay: String,
    /// Canonical digest of the overlay directory, pinned in the study design.
    pub sha256: String,
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
    pub variant: VariantIdentityRecord,
    pub task: TaskIdentityRecord,
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
pub struct VariantIdentityRecord {
    pub id: String,
    pub overlay: String,
    pub sha256: String,
}
#[derive(Debug, Serialize)]
pub struct TaskIdentityRecord {
    pub task_id: String,
    pub readme_sha256: String,
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
        let mut materials = std::collections::BTreeSet::new();
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
            ensure!(
                plain_relative(Path::new(&variant.overlay)),
                "variant {} overlay must be a non-empty relative path of plain components",
                variant.id
            );
            ensure!(
                sha256(&variant.sha256),
                "variant {} sha256 invalid",
                variant.id
            );
            ensure!(
                materials.insert(&variant.sha256),
                "variant {} reuses overlay material identity {}",
                variant.id,
                variant.sha256
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
    let verifier_sha256 = digest_file(&verify_path)?;
    verify_invariants(&cfg, &verifier, &verifier_sha256)?;
    ensure!(
        git::is_repo(root),
        "study root is not a git repository: {}",
        root.display()
    );
    // A study deliberately does not inspect HEAD.  Every cell starts with a fresh materialization
    // of the configured immutable base; a changed local branch cannot become a hidden variant.
    let base_tree = git::rev_parse(root, &format!("{}^{{tree}}", cfg.base_tree))?;
    ensure!(
        base_tree == cfg.base_tree,
        "study base_tree must name a tree object"
    );
    let parent = git::head(root)?;
    let diff = DiffMetrics {
        files: 0,
        additions: 0,
        deletions: 0,
    };
    let config_dir = config
        .parent()
        .context("study config has no parent directory")?;
    let prepared = prepare_variants(&cfg, config_dir, task_dir, &verifier_sha256)?;
    let mut lines = String::new();
    let mut failures = 0;
    for assignment in assignments(&cfg) {
        let variant = prepared
            .get(&assignment.variant)
            .expect("assignment variant exists");
        let frozen = git::detached_tree_worktree(root, &base_tree, &parent)?;
        let started = Instant::now();
        let output = gate::run(GateOpts {
            root: frozen.path().to_path_buf(),
            task_dir: variant.package.path().to_path_buf(),
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
            variant: VariantIdentityRecord {
                id: variant.spec.id.clone(),
                overlay: variant.spec.overlay.clone(),
                sha256: variant.actual_sha256.clone(),
            },
            task: TaskIdentityRecord {
                task_id: verifier.task_id.clone(),
                readme_sha256: digest_file(&variant.package.path().join("README.md"))?,
            },
            package_id: cfg.package_id.clone(),
            base_tree: cfg.base_tree.clone(),
            candidate_tree: base_tree.clone(),
            verifier: VerifierIdentityRecord {
                task_id: cfg.verifier.task_id.clone(),
                sha256: cfg.verifier.sha256.clone(),
            },
            harness: crate::HARNESS_FINGERPRINT.to_string(),
            image: cfg.image.clone(),
            agent: cfg.agent.clone(),
            model: cfg.model.clone(),
            normalized_claim: normalize_claim(&variant.spec.claim),
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

struct PreparedVariant<'a> {
    spec: &'a Variant,
    actual_sha256: String,
    package: tempfile::TempDir,
}

fn prepare_variants<'a>(
    cfg: &'a StudyConfig,
    config_dir: &Path,
    task_dir: &Path,
    verifier_sha256: &str,
) -> anyhow::Result<BTreeMap<String, PreparedVariant<'a>>> {
    let mut prepared = BTreeMap::new();
    for spec in &cfg.variants {
        let overlay = resolve_overlay(config_dir, &spec.overlay)?;
        let actual_sha256 = digest_overlay(&overlay)?;
        ensure!(
            actual_sha256 == spec.sha256,
            "variant {} overlay sha256 differs from study config",
            spec.id
        );
        let package = tempfile::Builder::new()
            .prefix("taskfmt-study-package-")
            .tempdir()
            .context("creating fresh study package")?;
        crate::ops::copy_tree(task_dir, package.path())?;
        crate::ops::copy_tree(&overlay, package.path())?;
        let materialized_verifier = package.path().join(crate::verifycfg::FILE_NAME);
        ensure!(
            digest_file(&materialized_verifier)? == verifier_sha256,
            "variant {} changes the invariant verifier",
            spec.id
        );
        let materialized = VerifyConfig::load(&materialized_verifier)?;
        verify_invariants(cfg, &materialized, verifier_sha256)?;
        prepared.insert(
            spec.id.clone(),
            PreparedVariant {
                spec,
                actual_sha256,
                package,
            },
        );
    }
    Ok(prepared)
}

fn resolve_overlay(config_dir: &Path, overlay: &str) -> anyhow::Result<PathBuf> {
    let rel = Path::new(overlay);
    ensure!(
        plain_relative(rel),
        "overlay is not a plain relative path: {overlay}"
    );
    let root = config_dir
        .canonicalize()
        .with_context(|| format!("canonicalizing study directory {}", config_dir.display()))?;
    let resolved = config_dir
        .join(rel)
        .canonicalize()
        .with_context(|| format!("canonicalizing variant overlay {overlay}"))?;
    ensure!(
        resolved.starts_with(&root) && resolved.is_dir(),
        "variant overlay must be a directory below the study config: {overlay}"
    );
    Ok(resolved)
}

fn plain_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

/// Canonical material identity for an overlay.  Directory, file, and byte boundaries all enter
/// the digest; symlinks and special files are refused so an identity cannot change at use time.
fn digest_overlay(root: &Path) -> anyhow::Result<String> {
    let mut entries = Vec::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(root)?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        ensure!(
            entry.file_type().is_dir() || entry.file_type().is_file(),
            "overlay contains non-regular entry: {}",
            rel.display()
        );
        ensure!(
            !rel.components()
                .any(|part| matches!(part, Component::Normal(name) if name == ".git")),
            "overlay must not contain .git: {}",
            rel.display()
        );
        entries.push((rel.to_path_buf(), entry.file_type().is_dir()));
    }
    ensure!(!entries.is_empty(), "overlay is empty");
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = Sha256::new();
    for (rel, is_dir) in entries {
        let text = rel
            .to_str()
            .context("overlay path is not UTF-8")?
            .replace(std::path::MAIN_SEPARATOR, "/");
        hasher.update(if is_dir { b"d" } else { b"f" });
        hasher.update((text.len() as u64).to_le_bytes());
        hasher.update(text.as_bytes());
        if !is_dir {
            let bytes = std::fs::read(root.join(rel))?;
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
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
                    overlay: "variants/a".into(),
                    sha256: "a".repeat(64),
                },
                Variant {
                    id: "b".into(),
                    claim: "B".into(),
                    overlay: "variants/b".into(),
                    sha256: "b".repeat(64),
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
        c = cfg();
        c.variants[1].sha256 = c.variants[0].sha256.clone();
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
        std::fs::write(
            root.join("src/after-base.rs"),
            "// must not enter a study cell\n",
        )
        .unwrap();
        git::add_all_including_ignored(&root).unwrap();
        git::commit(&root, "changed HEAD", false, false).unwrap();
        assert_ne!(git::rev_parse(&root, "HEAD^{tree}").unwrap(), base_tree);

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
        let original_readme = std::fs::read_to_string(task.join("README.md")).unwrap();
        let overlays = temp.path().join("overlays");
        let a = overlays.join("a");
        let b = overlays.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(
            a.join("README.md"),
            original_readme.replace(
                "Reject expired refresh tokens before session rotation",
                "Variant A expired refresh-token ordering",
            ),
        )
        .unwrap();
        std::fs::write(
            b.join("README.md"),
            original_readme.replace(
                "Reject expired refresh tokens before session rotation",
                "Variant B expired refresh-token ordering",
            ),
        )
        .unwrap();
        let a_sha = digest_overlay(&a).unwrap();
        let b_sha = digest_overlay(&b).unwrap();
        assert_ne!(a_sha, b_sha);
        let study_path = temp.path().join("study.toml");
        std::fs::write(&study_path, format!(
            "schema = \"study/v1\"\ncase_id = \"failure-capture\"\ninvariant_outcome = \"same verifier\"\npackage_id = \"TASK-042\"\nbase_tree = \"{base_tree}\"\nrepeats = 2\nblocks = 1\nrandom_seed = 1\nprimary_endpoint = \"gate\"\nartifact_policy = \"ndjson\"\nimage = \"none\"\nagent = \"test\"\nmodel = \"test\"\n[verifier]\ntask_id = \"TASK-042\"\nsha256 = \"{}\"\n[[variants]]\nid = \"a\"\nclaim = \"first\"\noverlay = \"overlays/a\"\nsha256 = \"{a_sha}\"\n[[variants]]\nid = \"b\"\nclaim = \"second\"\noverlay = \"overlays/b\"\nsha256 = \"{b_sha}\"\n", digest_file(&verifier_path).unwrap())).unwrap();
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
        assert!(records.iter().all(|record| {
            record["base_tree"] == base_tree
                && record["candidate_tree"] == base_tree
                && record["verifier"]["sha256"] == digest_file(&verifier_path).unwrap()
                && record["task"]["task_id"] == "TASK-042"
        }));
        let variants: BTreeMap<_, _> = records
            .iter()
            .map(|record| {
                (
                    record["variant"]["id"].as_str().unwrap(),
                    record["task"]["readme_sha256"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(variants.len(), 2);
        assert_ne!(variants["a"], variants["b"]);
        assert!(records.iter().all(|record| {
            let variant = record["variant"]["id"].as_str().unwrap();
            record["variant"]["sha256"].as_str()
                == Some(if variant == "a" { &a_sha } else { &b_sha })
        }));
    }
}
