//! The self-host driver (plan §4.6): the ledger, the fold over it, and the two subcommands that
//! read and write it.
//!
//! The tree is flat and its file set is fixed (TASK-114 D-002). `harness/src/cmds/selfhost.rs`
//! above it is thin dispatch; every flag is declared in [`cli`]; every ledger write goes through
//! [`ledger::append`] and through nothing else, which is plan §4.4 invariant 5's exact reading —
//! "no agent and no operator writes the ledger by any means other than the driver, and inside the
//! driver there is exactly one writer function".

pub mod cli;
pub mod hash;
pub mod ledger;
pub mod record;
pub mod state;
pub mod status;

use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::cmds::Ctx;
use crate::config::{ExperimentConfig, MANIFEST_NAME, Resolved};
use crate::redact;

/// Exit codes the driver returns itself (plan §4.6.18). clap owns 2; 64 and 69 stay unused.
pub const EXIT_OK: i32 = 0;
/// The subcommand's own negative verdict.
pub const EXIT_REFUSED: i32 = 1;
/// A terminal was reached.
pub const EXIT_TERMINAL: i32 = 4;
/// Missing input: the subject checkout, a named tree.
pub const EXIT_MISSING_INPUT: i32 = 66;

/// The three reserved pseudo-repo-ids of plan §3.3. **None is ever a fallback candidate.**
pub const RESERVED_REPO_IDS: [&str; 3] = ["calibration", "preflight", "runs"];

/// The one reserved id that carries a ledger (plan §4.6.2, and §4.6.21 edit 13 is why).
pub const PREFLIGHT: &str = "preflight";

/// Why a subcommand stopped early, and with which exit code.
#[derive(Debug)]
pub enum Stop {
    /// A negative verdict of the driver's own: exit 1, stderr, nothing on stdout.
    Refused(String),
    /// A named tree the driver needs and cannot find: exit 66.
    Missing(String),
    /// Anything else; `main` prints the chain and exits 1.
    Fault(anyhow::Error),
}

impl From<anyhow::Error> for Stop {
    fn from(err: anyhow::Error) -> Self {
        Stop::Fault(err)
    }
}

impl From<ledger::ReadError> for Stop {
    fn from(err: ledger::ReadError) -> Self {
        match err {
            ledger::ReadError::Refused(refusal) => Stop::Refused(refusal.0),
            ledger::ReadError::Fault(err) => Stop::Fault(err),
        }
    }
}

/// Map a subcommand's early stop onto the process exit code, printing the reason on stderr.
///
/// A refusal prints nothing on stdout, which is what makes `--sentinel`'s "exactly one line"
/// rule survive contact with a bad ledger.
pub fn finish(result: Result<i32, Stop>) -> anyhow::Result<i32> {
    match result {
        Ok(code) => Ok(code),
        Err(Stop::Refused(message)) => {
            redact::eemit(&message);
            Ok(EXIT_REFUSED)
        }
        Err(Stop::Missing(message)) => {
            redact::eemit(&message);
            Ok(EXIT_MISSING_INPUT)
        }
        Err(Stop::Fault(err)) => Err(err),
    }
}

/// Both manifests, both roots, and the reader the caller selected.
///
/// The driver loads BOTH manifests because confusing them is the failure mode this type prevents:
/// this repository's `experiment.toml` carries no `[github]`, no `[images]` and no `[runtime]`, so
/// every fallback default is wrong for the subject's purposes.
pub struct Driver {
    /// This repository. `Resolved::root` is the directory holding `experiment.toml`.
    pub meta: Resolved,
    /// The subject checkout and its own manifest.
    pub subject: Resolved,
    pub meta_root: PathBuf,
    pub subject_root: PathBuf,
    pub reader: ledger::Reader,
}

fn manifest_at(dir: &Path) -> anyhow::Result<Resolved> {
    let dir = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    let manifest = dir.join(MANIFEST_NAME);
    ExperimentConfig::load_resolved(&manifest)
        .with_context(|| format!("cannot load the manifest at {}", manifest.display()))
}

/// A directory counts as a checkout only when BOTH files are regular files under it.
fn is_subject_checkout(dir: &Path) -> bool {
    dir.join("harness").join("Cargo.toml").is_file() && dir.join(MANIFEST_NAME).is_file()
}

impl Driver {
    /// Resolve both roots. **Fails closed**: no guessing, and never a fall back to the cwd — a
    /// driver that silently judged this repository as the subject would report a clean fence over
    /// the wrong tree.
    pub fn resolve(ctx: &Ctx, common: &cli::CommonArgs) -> Result<Driver, Stop> {
        let meta = match &common.meta_root {
            Some(dir) => manifest_at(dir)?,
            None => ctx
                .load()
                .context("cannot load this repository's manifest")?,
        };
        let meta_root = meta.root.clone();

        let env_repo = std::env::var_os("TASKFORMAT_REPO")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let candidates: Vec<PathBuf> = match &common.subject_root {
            Some(dir) => vec![std::path::absolute(dir).unwrap_or_else(|_| dir.clone())],
            None => {
                let mut order = Vec::new();
                order.extend(env_repo);
                order.push(meta_root.join("..").join("task-format"));
                order.push(meta_root.join("subject"));
                order
            }
        };
        let subject_root = candidates
            .iter()
            .find(|dir| is_subject_checkout(dir))
            .cloned()
            .ok_or_else(|| {
                Stop::Missing(format!(
                    "no subject checkout: none of [{}] holds both harness/Cargo.toml and {}. Pass \
                     --subject-root <dir> or set $TASKFORMAT_REPO",
                    candidates
                        .iter()
                        .map(|dir| dir.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                    MANIFEST_NAME
                ))
            })?;
        let subject = manifest_at(&subject_root)?;

        let reader = if common.phase1 {
            ledger::Reader::Phase1Lenient
        } else {
            ledger::Reader::Strict
        };
        reader.announce();
        Ok(Driver {
            meta,
            subject,
            meta_root,
            subject_root,
            reader,
        })
    }

    /// `<meta_root>/selfhost/state`.
    pub fn state_dir(&self) -> PathBuf {
        self.meta_root.join("selfhost").join("state")
    }

    /// `<meta_root>/selfhost/state/<repo-id>/ledger.jsonl`.
    pub fn ledger_path(&self, repo_id: &str) -> PathBuf {
        self.state_dir().join(repo_id).join("ledger.jsonl")
    }

    /// Which ledger to read or write (TASK-114 D-008).
    ///
    /// `--repo-id` decides it when given — `preflight` is accepted, the other two reserved names
    /// are refused. Otherwise the single non-reserved candidate; otherwise the lexicographically
    /// greatest, which is chronological because every repo id ends in a `YYYYMMDD-HHMMSS` stamp.
    /// A modification-time rule was rejected: `git checkout` rewrites mtimes.
    pub fn select_repo_id(&self, explicit: Option<&str>, preflight: bool) -> Result<String, Stop> {
        if preflight {
            if let Some(id) = explicit.filter(|id| *id != PREFLIGHT) {
                return Err(Stop::Refused(format!(
                    "LEDGER-REFUSED repo-id-conflict --preflight selects {PREFLIGHT} but --repo-id \
                     names {id}"
                )));
            }
            return Ok(PREFLIGHT.to_string());
        }
        if let Some(id) = explicit {
            if id == PREFLIGHT {
                return Ok(id.to_string());
            }
            if RESERVED_REPO_IDS.contains(&id) {
                return Err(Stop::Refused(format!(
                    "LEDGER-REFUSED reserved-repo-id {id} carries no ledger (plan §3.3)"
                )));
            }
            return Ok(id.to_string());
        }
        let state = self.state_dir();
        let mut candidates: Vec<String> = std::fs::read_dir(&state)
            .with_context(|| format!("cannot read {}", state.display()))
            .map_err(Stop::Fault)?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.join("ledger.jsonl").is_file())
            .filter_map(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
            })
            .filter(|name| !RESERVED_REPO_IDS.contains(&name.as_str()))
            .collect();
        candidates.sort();
        candidates.pop().ok_or_else(|| {
            Stop::Missing(format!(
                "no ledger under {}: name one with --repo-id",
                state.display()
            ))
        })
    }

    /// A reserved pseudo-repo-id: the fold is refused outright for it (plan §4.6.4).
    pub fn is_pseudo(repo_id: &str) -> bool {
        RESERVED_REPO_IDS.contains(&repo_id)
    }

    /// The subject corpus, enumerated from the subject manifest's `tasks_dir` and never from a
    /// hard-coded 1..7.
    pub fn subject_corpus(&self) -> anyhow::Result<Vec<String>> {
        let dirs = crate::cmds::all_task_dirs(&self.subject.tasks_dir())?;
        Ok(dirs
            .iter()
            .filter_map(|path| path.file_name())
            .map(|name| name.to_string_lossy().to_string())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_reserved_names_are_the_plans_three() {
        assert_eq!(RESERVED_REPO_IDS.len(), 3);
        for name in ["calibration", "preflight", "runs"] {
            assert!(Driver::is_pseudo(name), "{name} is reserved");
        }
        assert!(!Driver::is_pseudo("taskfmt-experiment-20260829-192730"));
    }

    #[test]
    fn a_subject_checkout_needs_both_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(!is_subject_checkout(root));
        std::fs::create_dir_all(root.join("harness")).unwrap();
        std::fs::write(root.join("harness").join("Cargo.toml"), "x").unwrap();
        assert!(!is_subject_checkout(root), "one file is not a checkout");
        std::fs::write(root.join(MANIFEST_NAME), "x").unwrap();
        assert!(is_subject_checkout(root));
    }

    #[test]
    fn a_refusal_exits_one_and_a_missing_tree_exits_sixty_six() {
        assert_eq!(
            finish(Err(Stop::Refused("LEDGER-REFUSED x".into()))).unwrap(),
            EXIT_REFUSED
        );
        assert_eq!(finish(Err(Stop::Missing("no tree".into()))).unwrap(), 66);
        assert_eq!(finish(Ok(EXIT_TERMINAL)).unwrap(), 4);
    }
}
