//! The dispatch-time gate-identity check: the digest, the comparison, and the refusal.
//!
//! Hermetic by construction. The image side of the comparison is a parameter of `dispatch_one` and
//! `run::run`, so every test here supplies its own reader and none of them consults a daemon, an
//! image or a network. The one test that drives a real dispatch proves the refusal fires before
//! anything is created on disk, which is the property the ordering criterion over `run.rs` can only
//! assert about text.
//!
//! This file is a declared `[[test]]` target outside `tests/` (`harness/Cargo.toml`). It is
//! deliberately not part of the hash input set: the image build stage never receives `checks/`, so
//! hashing it would make the host and every image disagree permanently.

use std::path::Path;

use taskfmt::cmds::Ctx;
use taskfmt::cmds::run::require_image_fingerprint_match;
use taskfmt::config::{ExperimentConfig, Resolved};
use taskfmt::interactive::Interaction;
use taskfmt::ops::docker::ImageFingerprint;

const MANIFEST: &str = r#"
schema = "experiment/v1"
[paths]
tasks_dir = "tasks"
runs_dir = "runs"
[agents.default]
profile = "zai-flash"
[agents.profiles.zai-flash]
kind = "claude"
model = "glm-5.3-flash"
effort = "low"
image = "harness-claude:latest"
"#;

/// A reader that answers with a fixed value, whatever the image.
struct Fixed(String);

impl ImageFingerprint for Fixed {
    fn image_fingerprint(&self, _image: &str) -> anyhow::Result<String> {
        Ok(self.0.clone())
    }
}

/// A reader that cannot answer at all — an absent image, a dead daemon, an image too old to know
/// the subcommand. Every one of those is a mismatch, never a reason to proceed.
struct Unreadable;

impl ImageFingerprint for Unreadable {
    fn image_fingerprint(&self, image: &str) -> anyhow::Result<String> {
        anyhow::bail!("no such image: {image}")
    }
}

/// The value that is never this binary's: 64 lowercase hex digits and not the host constant.
fn other_than_host() -> String {
    let other = "9".repeat(64);
    assert_ne!(other, taskfmt::HARNESS_FINGERPRINT);
    other
}

/// The compiled-in constant is the digest of the hash input set, recomputed here from the crate
/// directory this test was built from — the same function the build script ran, over the same four
/// inputs. A constant that was a literal, or a digest over a different input set, fails.
#[test]
fn host_constant_is_the_digest_of_the_hash_input_set() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let recomputed = taskfmt::fingerprint::fingerprint(crate_dir).unwrap();
    assert_eq!(
        recomputed,
        taskfmt::HARNESS_FINGERPRINT,
        "the compiled-in value must be the digest of Cargo.toml, Cargo.lock, build.rs and src/"
    );
    assert_eq!(taskfmt::HARNESS_FINGERPRINT.len(), 64);
    assert!(
        taskfmt::HARNESS_FINGERPRINT
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
        "{}",
        taskfmt::HARNESS_FINGERPRINT
    );

    // and it is a function of the CONTENT, not of the size: a same-length edit moves it
    let scratch = tempfile::tempdir().unwrap();
    let copy = scratch.path().join("copy");
    std::fs::create_dir_all(copy.join("src")).unwrap();
    for name in ["Cargo.toml", "Cargo.lock", "build.rs"] {
        std::fs::copy(crate_dir.join(name), copy.join(name)).unwrap();
    }
    let lib = std::fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    std::fs::write(copy.join("src/lib.rs"), &lib).unwrap();
    let one_file = taskfmt::fingerprint::fingerprint(&copy).unwrap();
    let edited = lib.replacen('a', "A", 1);
    assert_eq!(
        edited.len(),
        lib.len(),
        "the drift must not change the size"
    );
    assert_ne!(edited, lib);
    std::fs::write(copy.join("src/lib.rs"), &edited).unwrap();
    assert_ne!(
        taskfmt::fingerprint::fingerprint(&copy).unwrap(),
        one_file,
        "a same-length change of the bytes must move the digest"
    );
}

/// Two equal values are the same build: the dispatch proceeds.
#[test]
fn equal_values_are_accepted() {
    require_image_fingerprint_match(
        &Fixed(taskfmt::HARNESS_FINGERPRINT.to_string()),
        "harness-claude:latest",
    )
    .unwrap();
}

/// A difference is refused, and the message carries everything the operator needs: both values and
/// the command that repairs the image side.
#[test]
fn mismatch_names_both_values_and_the_remedy() {
    let other = other_than_host();
    let err = require_image_fingerprint_match(&Fixed(other.clone()), "harness-claude:latest")
        .unwrap_err()
        .to_string();
    assert!(err.contains(taskfmt::HARNESS_FINGERPRINT), "{err}");
    assert!(err.contains(&other), "{err}");
    assert!(err.contains("taskfmt build-images"), "{err}");
    assert!(err.contains("harness-claude:latest"), "{err}");
}

/// A reader that cannot answer never passes: the refusal names the image and both remedies.
#[test]
fn unreadable_image_value_is_refused() {
    let err = require_image_fingerprint_match(&Unreadable, "harness-claude:latest")
        .unwrap_err()
        .to_string();
    assert!(err.contains("harness-claude:latest"), "{err}");
    assert!(err.contains("taskfmt build-images"), "{err}");
    assert!(err.contains("cargo install"), "{err}");
}

/// The whole point, driven for real: `run::run` against a reader whose value differs from this
/// binary's fails before the run directory exists — before the clone, so the message names the
/// fingerprints and not the repository. No daemon, no image, no network.
#[test]
fn dispatch_refuses_before_creating_the_run_directory() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("experiment.toml"), MANIFEST).unwrap();
    let (cfg, root) = ExperimentConfig::load(&dir.path().join("experiment.toml")).unwrap();
    let resolved = Resolved::new(&root, cfg);
    let task = resolved.tasks_dir().join("TASK-001");
    std::fs::create_dir_all(&task).unwrap();
    std::fs::write(task.join("README.md"), "x").unwrap();
    let ctx = Ctx {
        config_path: dir.path().join("experiment.toml"),
        verbose: false,
        interaction: Interaction::new(true, true),
    };

    let other = other_than_host();
    let err = taskfmt::cmds::run::run(
        &ctx,
        "TASK-001",
        Some("https://github.invalid/taskfmt-tests/no-such-repo.git"),
        None,
        None,
        None,
        false,
        None,
        None,
        false,
        &Fixed(other.clone()),
    )
    .unwrap_err()
    .to_string();

    assert!(err.contains(taskfmt::HARNESS_FINGERPRINT), "{err}");
    assert!(err.contains(&other), "{err}");
    assert!(err.contains("taskfmt build-images"), "{err}");
    assert!(
        !err.contains("cloning"),
        "the refusal fires before the clone: {err}"
    );
    let runs = resolved.runs_dir();
    let created: Vec<std::path::PathBuf> = std::fs::read_dir(&runs)
        .map(|entries| entries.map(|entry| entry.unwrap().path()).collect())
        .unwrap_or_default();
    assert!(created.is_empty(), "no run directory: {created:?}");
}
