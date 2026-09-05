//! Contract helpers for the opt-in Docker end-to-end executable.
//!
//! `libtest` has no runtime skip result: a test body that returns early is reported PASS. The
//! custom executable uses this pure decision layer so unavailable Docker is visibly `SKIP`.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, bail};

pub const PROOF_SCHEMA: &str = "taskfmt/docker-itest-proof/v1";
pub const CHECK_PREREQS: &str = "container_runs_prereq_stage_and_stays_up";
pub const CHECK_ARCH: &str = "arch_detection_agrees_with_the_server";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preflight {
    Run,
    Skip(&'static str),
}

pub fn preflight(enabled: bool, daemon_available: bool, image_available: bool) -> Preflight {
    if !enabled {
        Preflight::Skip("TASKFMT_ITEST_DOCKER is not 1")
    } else if !daemon_available {
        Preflight::Skip("docker daemon unavailable")
    } else if !image_available {
        Preflight::Skip("harness-base:latest is not built")
    } else {
        Preflight::Run
    }
}

/// Write only after every enabled body has passed. `create_new` rejects stale evidence.
pub fn write_proof(path: &Path, checks: &[&str]) -> Result<()> {
    if checks != [CHECK_PREREQS, CHECK_ARCH] {
        bail!("Docker proof requires exactly the complete ordered check set")
    }
    let text = format!(
        "{{\"schema\":\"{PROOF_SCHEMA}\",\"enabled\":true,\"checks\":[{{\"id\":\"{CHECK_PREREQS}\",\"result\":\"PASS\"}},{{\"id\":\"{CHECK_ARCH}\",\"result\":\"PASS\"}}]}}\n"
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("creating fresh Docker integration proof {}", path.display()))?;
    file.write_all(text.as_bytes())
        .context("writing Docker integration proof")?;
    file.sync_all()
        .context("syncing Docker integration proof")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_skips_each_unavailable_state_and_runs_only_when_ready() {
        assert_eq!(
            preflight(false, true, true),
            Preflight::Skip("TASKFMT_ITEST_DOCKER is not 1")
        );
        assert_eq!(
            preflight(true, false, true),
            Preflight::Skip("docker daemon unavailable")
        );
        assert_eq!(
            preflight(true, true, false),
            Preflight::Skip("harness-base:latest is not built")
        );
        assert_eq!(preflight(true, true, true), Preflight::Run);
    }

    #[test]
    fn proof_is_complete_ordered_and_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let proof = dir.path().join("proof.json");
        write_proof(&proof, &[CHECK_PREREQS, CHECK_ARCH]).unwrap();
        assert_eq!(
            std::fs::read_to_string(&proof).unwrap(),
            format!(
                "{{\"schema\":\"{PROOF_SCHEMA}\",\"enabled\":true,\"checks\":[{{\"id\":\"{CHECK_PREREQS}\",\"result\":\"PASS\"}},{{\"id\":\"{CHECK_ARCH}\",\"result\":\"PASS\"}}]}}\n"
            )
        );
        assert!(write_proof(&proof, &[CHECK_PREREQS, CHECK_ARCH]).is_err());
        assert!(write_proof(&dir.path().join("wrong.json"), &[CHECK_ARCH, CHECK_PREREQS]).is_err());
    }
}
