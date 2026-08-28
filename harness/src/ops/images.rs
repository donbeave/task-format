//! Image builds and the postgres preload.
//!
//! Build order matters: `harness-taskfmt` (the crate) → `harness-base` (`COPY --from=harness-taskfmt`)
//! → `harness-claude` / `harness-codex` on top of the base.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;

use crate::config::{ExperimentConfig, Resolved};

use super::docker::{self, BuildOpts};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentFilter {
    Claude,
    Codex,
    All,
}

/// Refuse to build an image that would be missing the baked postgres tarball.
fn require_preload_tar(harness_dir: &Path) -> anyhow::Result<PathBuf> {
    let tar = harness_dir.join("images/preload/postgres.tar");
    let meta = std::fs::metadata(&tar).with_context(|| {
        format!(
            "{} missing or empty — run `taskfmt preload` first",
            tar.display()
        )
    })?;
    if meta.len() == 0 {
        anyhow::bail!("{} is empty — run `taskfmt preload` first", tar.display());
    }
    Ok(tar)
}

/// Build the four images in dependency order.
pub fn build_images(
    cfg: &ExperimentConfig,
    resolved: &Resolved,
    filter: AgentFilter,
    no_cache: bool,
    verbose: bool,
) -> anyhow::Result<()> {
    let harness_dir = resolved.root.join("harness");
    let images_dir = harness_dir.join("images");
    let arch = docker::target_arch()?;
    crate::redact::emit(&format!("== target arch: {arch}"));

    // 1. the taskfmt binary image (build context = harness/, the crate root)
    crate::redact::emit(&format!("== {}", cfg.images.taskfmt));
    docker::build(&BuildOpts {
        dockerfile: images_dir.join("taskfmt/Dockerfile"),
        context: harness_dir.clone(),
        tag: cfg.images.taskfmt.clone(),
        target_arch: arch.clone(),
        build_args: Vec::new(),
        no_cache,
    })?;

    // 2. the base image, which COPYs the taskfmt binary out of it
    require_preload_tar(&harness_dir)?;
    crate::redact::emit(&format!("== {}", cfg.images.base));
    docker::build(&BuildOpts {
        dockerfile: images_dir.join("base/Dockerfile"),
        context: harness_dir.clone(),
        tag: cfg.images.base.clone(),
        target_arch: arch.clone(),
        build_args: Vec::new(),
        no_cache,
    })?;

    // 3. the agent layers
    let agents: Vec<(&str, &str, &str, &str)> = match filter {
        AgentFilter::Claude => vec![(
            "claude",
            &cfg.images.claude,
            "CLAUDE_CODE_VERSION",
            "2.1.250",
        )],
        AgentFilter::Codex => vec![("codex", &cfg.images.codex, "CODEX_VERSION", "0.150.1")],
        AgentFilter::All => vec![
            (
                "claude",
                &cfg.images.claude,
                "CLAUDE_CODE_VERSION",
                "2.1.250",
            ),
            ("codex", &cfg.images.codex, "CODEX_VERSION", "0.150.1"),
        ],
    };
    for (kind, tag, arg, default) in agents {
        if verbose {
            crate::redact::emit(&format!("== harness-{kind} ({arg}={default})"));
        } else {
            crate::redact::emit(&format!("== harness-{kind}"));
        }
        docker::build(&BuildOpts {
            dockerfile: images_dir.join(format!("{kind}/Dockerfile")),
            context: harness_dir.clone(),
            tag: tag.to_string(),
            target_arch: arch.clone(),
            build_args: vec![(arg.to_string(), default.to_string())],
            no_cache,
        })?;
    }

    crate::redact::emit(&format!(
        "== built: {} {} {}",
        cfg.images.taskfmt, cfg.images.base, cfg.images.claude
    ));
    Ok(())
}

pub const PRELOAD_REF: &str = "postgres:16-alpine";
pub const PRELOAD_CONTAINER: &str = "prereq-postgres";

/// Pull the postgres image, pin its digest (write-once), and `docker save` the tarball.
pub fn preload(harness_dir: &Path) -> anyhow::Result<()> {
    let preload_dir = harness_dir.join("images/preload");
    std::fs::create_dir_all(&preload_dir)?;
    let digest_file = preload_dir.join("postgres.digest");
    let tar = preload_dir.join("postgres.tar");

    if digest_file.exists() && std::fs::metadata(&digest_file)?.len() > 0 {
        let pinned = std::fs::read_to_string(&digest_file)?.trim().to_string();
        crate::redact::emit(&format!("== pull pinned: {pinned}"));
        super::check(
            Command::new("docker").args(["pull", &pinned]),
            &format!("docker pull {pinned}"),
        )?;
        super::check(
            Command::new("docker").args(["tag", &pinned, PRELOAD_REF]),
            &format!("docker tag {pinned} {PRELOAD_REF}"),
        )?;
    } else {
        crate::redact::emit(&format!(
            "== no digest pin yet — pulling {PRELOAD_REF} (this run records the digest)"
        ));
        super::check(
            Command::new("docker").args(["pull", PRELOAD_REF]),
            &format!("docker pull {PRELOAD_REF}"),
        )?;
        let digest = super::capture(Command::new("docker").args([
            "image",
            "inspect",
            "--format",
            "{{index .RepoDigests 0}}",
            PRELOAD_REF,
        ]))?;
        if !digest.ok() {
            anyhow::bail!(
                "cannot read the RepoDigest of {PRELOAD_REF}: {}",
                digest.stderr.trim_end()
            );
        }
        let digest = digest.stdout.trim().to_string();
        super::write_file(&digest_file, &format!("{digest}\n"))?;
    }

    let digest = std::fs::read_to_string(&digest_file)?.trim().to_string();
    crate::redact::emit(&format!("== digest: {digest}"));
    crate::redact::emit(&format!("== save {PRELOAD_REF} -> {}", tar.display()));
    super::check(
        Command::new("docker")
            .args(["save", PRELOAD_REF, "-o"])
            .arg(&tar),
        &format!("docker save {PRELOAD_REF}"),
    )?;
    let bytes = std::fs::metadata(&tar)?.len();
    crate::redact::emit(&format!(
        "== wrote {} ({}); prereqs will docker load it",
        tar.display(),
        human(bytes)
    ));
    Ok(())
}

fn human(bytes: u64) -> String {
    let mib = bytes / (1024 * 1024);
    if mib >= 1024 {
        format!("{:.1}GiB", mib as f64 / 1024.0)
    } else {
        format!("{mib}MiB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_tar_is_reported_before_any_build() {
        let dir = tempfile::tempdir().unwrap();
        let err = require_preload_tar(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("taskfmt preload"), "{err:#}");
        std::fs::create_dir_all(dir.path().join("images/preload")).unwrap();
        std::fs::write(dir.path().join("images/preload/postgres.tar"), []).unwrap();
        let err = require_preload_tar(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("is empty"), "{err:#}");
    }

    #[test]
    fn human_sizes() {
        assert_eq!(human(0), "0MiB");
        assert_eq!(human(200 * 1024 * 1024), "200MiB");
        assert_eq!(human(2 * 1024 * 1024 * 1024), "2.0GiB");
    }
}
