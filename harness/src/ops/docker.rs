//! docker CLI wrapper: image builds, the persistent run container, exec into it.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;

use super::{Captured, capture, check};

/// Server architecture (`docker version --format '{{.Server.Arch}}'`), falling back to `uname -m`.
pub fn server_arch() -> anyhow::Result<String> {
    if let Ok(out) =
        capture(Command::new("docker").args(["version", "--format", "{{.Server.Arch}}"]))
        && out.ok()
    {
        let arch = out.stdout.trim().to_string();
        if !arch.is_empty() {
            return Ok(arch);
        }
    }
    let out = capture(Command::new("uname").arg("-m")).context("uname -m failed")?;
    Ok(out.stdout.trim().to_string())
}

/// Docker build target arch for release assets: `x86_64|amd64 → amd64`, `aarch64|arm64 → arm64`.
pub fn target_arch() -> anyhow::Result<String> {
    let arch = server_arch()?;
    match arch.as_str() {
        "x86_64" | "amd64" => Ok("amd64".to_string()),
        "aarch64" | "arm64" => Ok("arm64".to_string()),
        other => anyhow::bail!("unsupported architecture: {other}"),
    }
}

#[derive(Debug, Clone)]
pub struct BuildOpts {
    pub dockerfile: PathBuf,
    pub context: PathBuf,
    pub tag: String,
    pub target_arch: String,
    pub build_args: Vec<(String, String)>,
    pub no_cache: bool,
}

pub fn build(opts: &BuildOpts) -> anyhow::Result<()> {
    let mut cmd = Command::new("docker");
    cmd.args(["build", "--progress=plain"])
        .arg("-f")
        .arg(&opts.dockerfile)
        .arg("--build-arg")
        .arg(format!("TARGETARCH={}", opts.target_arch))
        .arg("-t")
        .arg(&opts.tag);
    for (key, value) in &opts.build_args {
        cmd.arg("--build-arg").arg(format!("{key}={value}"));
    }
    if opts.no_cache {
        cmd.arg("--no-cache");
    }
    cmd.arg(&opts.context);
    check(&mut cmd, &format!("docker build {}", opts.tag))?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct RunSpec {
    pub name: String,
    pub image: String,
    pub mounts: Vec<Mount>,
    pub env: Vec<(String, String)>,
    /// Path to a 0600 env file holding the secrets; deleted right after the run.
    pub env_file: Option<PathBuf>,
    pub memory: String,
    pub cpus: f32,
    pub pids_limit: i64,
}

#[derive(Debug, Clone)]
pub struct Mount {
    pub host: PathBuf,
    pub container: String,
    pub read_only: bool,
}

impl Mount {
    pub fn rw(host: &Path, container: &str) -> Self {
        Self {
            host: host.to_path_buf(),
            container: container.to_string(),
            read_only: false,
        }
    }

    pub fn ro(host: &Path, container: &str) -> Self {
        Self {
            host: host.to_path_buf(),
            container: container.to_string(),
            read_only: true,
        }
    }

    fn flag(&self) -> String {
        let mut out = format!("{}:{}", self.host.display(), self.container);
        if self.read_only {
            out.push_str(":ro");
        }
        out
    }
}

/// `docker run -d --privileged` — named, persistent, **no** `--rm` (hard rule), no `-t`
/// (the herdr server needs no TTY).
pub fn run_detached(spec: &RunSpec) -> anyhow::Result<String> {
    let mut cmd = Command::new("docker");
    cmd.args(["run", "-d", "--privileged", "--name"])
        .arg(&spec.name);
    for mount in &spec.mounts {
        cmd.arg("-v").arg(mount.flag());
    }
    for (key, value) in &spec.env {
        cmd.arg("-e").arg(format!("{key}={value}"));
    }
    if let Some(env_file) = &spec.env_file {
        cmd.arg("--env-file").arg(env_file);
    }
    cmd.args([
        "--memory",
        &spec.memory,
        "--cpus",
        &format!("{}", spec.cpus),
        "--pids-limit",
    ])
    .arg(spec.pids_limit.to_string())
    .arg(&spec.image);
    let captured = check(&mut cmd, &format!("docker run {}", spec.name))?;
    Ok(captured.stdout.trim().to_string())
}

/// Run a command inside the container, optionally as another user, with extra env.
pub fn exec(
    container: &str,
    user: Option<&str>,
    env: &[(String, String)],
    args: &[String],
    tty: bool,
) -> anyhow::Result<Captured> {
    let mut cmd = Command::new("docker");
    cmd.arg("exec");
    if tty {
        cmd.arg("-it");
    }
    if let Some(user) = user {
        cmd.arg("-u").arg(user);
    }
    cmd.arg("-e").arg(format!(
        "TERM={}",
        std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".into())
    ));
    for (key, value) in env {
        cmd.arg("-e").arg(format!("{key}={value}"));
    }
    cmd.arg(container).args(args);
    capture(&mut cmd).context("cannot spawn docker exec")
}

/// Same as [`exec`] but errors on a non-zero exit.
pub fn exec_ok(
    container: &str,
    user: Option<&str>,
    env: &[(String, String)],
    args: &[String],
) -> anyhow::Result<Captured> {
    let captured = exec(container, user, env, args, false)?;
    if !captured.ok() {
        anyhow::bail!(
            "docker exec {} failed (rc={}):\n{}",
            container,
            captured.status,
            crate::redact::scrub(captured.stderr.trim_end())
        );
    }
    Ok(captured)
}

/// The host source of a bind mount at `destination` (e.g. `/work`), or `None` when the container
/// does not exist or has no such mount. Used to find a run's directory from its container alone,
/// when no manifest is discoverable from the cwd (`cmds::load_for_run`).
pub fn inspect_mount_source(container: &str, destination: &str) -> Option<PathBuf> {
    let template = format!(
        "{{{{range .Mounts}}}}{{{{if eq .Destination \"{destination}\"}}}}{{{{.Source}}}}{{{{end}}}}{{{{end}}}}"
    );
    let out = capture(Command::new("docker").args(["inspect", "-f", &template, container])).ok()?;
    if !out.ok() {
        return None;
    }
    let source = out.stdout.trim();
    if source.is_empty() {
        None
    } else {
        Some(PathBuf::from(source))
    }
}

pub fn is_running(container: &str) -> bool {
    capture(Command::new("docker").args(["inspect", "-f", "{{.State.Running}}", container]))
        .map(|out| out.ok() && out.stdout.trim() == "true")
        .unwrap_or(false)
}

pub fn exists(container: &str) -> bool {
    capture(Command::new("docker").args(["inspect", "-f", "{{.Id}}", container]))
        .map(|out| out.ok())
        .unwrap_or(false)
}

pub fn start(container: &str) -> anyhow::Result<()> {
    let _ = capture(Command::new("docker").arg("start").arg(container));
    Ok(())
}

pub fn image_exists(image: &str) -> bool {
    capture(Command::new("docker").args(["image", "inspect", image]))
        .map(|out| out.ok())
        .unwrap_or(false)
}

/// Read a file inside the container (`docker exec … cat`). Empty when it does not exist.
pub fn read_file(container: &str, path: &str) -> Option<String> {
    let out = exec(
        container,
        Some("root"),
        &[],
        &["cat".to_string(), path.to_string()],
        false,
    )
    .ok()?;
    if out.ok() { Some(out.stdout) } else { None }
}

/// Poll `path` inside the container until it exists or the deadline passes. Returns whether it was
/// seen in time.
pub fn wait_for_file(
    container: &str,
    path: &str,
    deadline: std::time::Duration,
) -> anyhow::Result<bool> {
    let started = std::time::Instant::now();
    while started.elapsed() < deadline {
        if read_file(container, path).is_some() {
            return Ok(true);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Ok(false)
}
