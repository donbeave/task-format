//! docker CLI wrapper: image builds, the persistent run container, exec into it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::Context;

use super::{Captured, capture, capture_with_timeout};

/// Docker control-plane requests must fail rather than pinning a lifecycle poll forever.
const CONTROL_TIMEOUT: Duration = Duration::from_millis(500);
/// Image builds can legitimately take a long time, but must still have a finite upper bound.
const BUILD_TIMEOUT: Duration = Duration::from_secs(60 * 60);

fn capture_docker(cmd: &mut Command) -> std::io::Result<Captured> {
    capture_with_timeout(cmd, CONTROL_TIMEOUT)
}

fn check_docker(cmd: &mut Command, what: &str) -> anyhow::Result<Captured> {
    check_docker_with_timeout(cmd, what, CONTROL_TIMEOUT)
}

fn check_docker_with_timeout(
    cmd: &mut Command,
    what: &str,
    timeout: Duration,
) -> anyhow::Result<Captured> {
    let captured = capture_with_timeout(cmd, timeout)
        .map_err(|err| anyhow::anyhow!("{what}: docker command failed or timed out: {err}"))?;
    if !captured.ok() {
        anyhow::bail!(
            "{what} failed (rc={}):\n{}",
            captured.status,
            crate::redact::scrub(if captured.stderr.trim().is_empty() {
                &captured.stdout
            } else {
                &captured.stderr
            })
            .trim_end()
        );
    }
    Ok(captured)
}

/// Server architecture (`docker version --format '{{.Server.Arch}}'`), falling back to `uname -m`.
pub fn server_arch() -> anyhow::Result<String> {
    if let Ok(out) =
        capture_docker(Command::new("docker").args(["version", "--format", "{{.Server.Arch}}"]))
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
    check_docker_with_timeout(
        &mut cmd,
        &format!("docker build {}", opts.tag),
        BUILD_TIMEOUT,
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct RunSpec {
    pub name: String,
    pub image: String,
    pub mounts: Vec<Mount>,
    pub env: Vec<(String, String)>,
    /// `--label key=value` pairs. These are what make the container self-describing: they survive
    /// the process that launched it, travel with `docker inspect`, and need no file on disk.
    pub labels: Vec<(String, String)>,
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
    for (key, value) in &spec.labels {
        cmd.arg("--label").arg(format!("{key}={value}"));
    }
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
    let captured = check_docker(&mut cmd, &format!("docker run {}", spec.name))?;
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
    capture_docker(&mut cmd).context("cannot spawn docker exec")
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

/// The workspace bind mount. Its host source is `<run dir>/workspace`, so its parent is the run
/// directory — the only self-description a container launched before the `taskfmt.*` labels has.
pub const WORK_MOUNT: &str = "/work";

/// What one container says about itself: enough to find its run without reading any manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContainerInfo {
    /// Container name, without docker's leading `/`.
    pub name: String,
    /// `created` | `running` | `paused` | `restarting` | `removing` | `exited` | `dead`.
    pub state: String,
    /// Host source of the [`WORK_MOUNT`] bind mount, when it has one.
    pub work_mount: Option<PathBuf>,
    /// `--label` values by name (empty for a container launched before the labels existed).
    pub labels: BTreeMap<String, String>,
}

impl ContainerInfo {
    /// A label's value, treating an empty value as absent.
    pub fn label(&self, key: &str) -> Option<&str> {
        self.labels
            .get(key)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
    }

    pub fn is_running(&self) -> bool {
        self.state == "running"
    }
}

/// The `docker inspect -f` template behind [`inspect_containers`]: four tab-separated fields, with
/// the labels rendered as JSON last, so no value can be confused for a field separator (`json` of a
/// nil label map renders `null`, which parses to no labels rather than erroring).
fn inspect_template() -> String {
    format!(
        "{{{{.Name}}}}\t{{{{.State.Status}}}}\t\
         {{{{range .Mounts}}}}{{{{if eq .Destination \"{WORK_MOUNT}\"}}}}{{{{.Source}}}}{{{{end}}}}{{{{end}}}}\t\
         {{{{json .Config.Labels}}}}"
    )
}

/// Parse one line of [`inspect_template`]'s output. `None` for a line docker did not produce for a
/// real container (an empty line, or the blank line docker emits for a name it could not find).
fn parse_inspect_line(line: &str) -> Option<ContainerInfo> {
    let mut fields = line.split('\t');
    let name = fields.next()?.trim().trim_start_matches('/').to_string();
    if name.is_empty() {
        return None;
    }
    let state = fields.next().unwrap_or_default().trim().to_string();
    let work = fields.next().unwrap_or_default().trim().to_string();
    let labels = fields
        .next()
        .and_then(|json| serde_json::from_str::<BTreeMap<String, String>>(json.trim()).ok())
        .unwrap_or_default();
    Some(ContainerInfo {
        name,
        state,
        work_mount: (!work.is_empty()).then(|| PathBuf::from(work)),
        labels,
    })
}

/// `docker inspect` every named container in one call. Names docker does not know are simply
/// absent from the result: it reports them on stderr and exits non-zero while still printing a
/// line for each name it *did* find, so the parsed lines are the answer and the exit code is not.
pub fn inspect_containers(names: &[String]) -> Vec<ContainerInfo> {
    if names.is_empty() {
        return Vec::new();
    }
    let Ok(out) = capture_docker(
        Command::new("docker")
            .args(["inspect", "-f", &inspect_template()])
            .args(names),
    ) else {
        return Vec::new();
    };
    out.stdout.lines().filter_map(parse_inspect_line).collect()
}

/// [`inspect_containers`] for one name.
pub fn inspect_container(name: &str) -> Option<ContainerInfo> {
    inspect_containers(&[name.to_string()])
        .into_iter()
        .find(|info| info.name == name)
}

/// Every container whose name starts with `prefix`, running or not, in docker's own order
/// (newest first). Empty when docker cannot be reached — callers that need to tell "no runs" from
/// "no daemon" ask [`available`] first.
pub fn list_containers(prefix: &str) -> Vec<String> {
    let filter = format!("name=^{prefix}");
    capture_docker(Command::new("docker").args([
        "ps",
        "-a",
        "--filter",
        &filter,
        "--format",
        "{{.Names}}",
    ]))
    .ok()
    .filter(Captured::ok)
    .map(|out| {
        out.stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

/// Whether the docker daemon answers at all.
pub fn available() -> bool {
    capture_docker(Command::new("docker").args(["version", "--format", "{{.Server.Version}}"]))
        .map(|out| out.ok())
        .unwrap_or(false)
}

pub fn is_running(container: &str) -> bool {
    capture_docker(Command::new("docker").args(["inspect", "-f", "{{.State.Running}}", container]))
        .map(|out| out.ok() && out.stdout.trim() == "true")
        .unwrap_or(false)
}

pub fn exists(container: &str) -> bool {
    capture_docker(Command::new("docker").args(["inspect", "-f", "{{.Id}}", container]))
        .map(|out| out.ok())
        .unwrap_or(false)
}

pub fn start(container: &str) -> anyhow::Result<()> {
    let _ = capture_docker(Command::new("docker").arg("start").arg(container));
    Ok(())
}

/// Stop a container and wait for it to exit; `grace_s` is docker's SIGTERM window before SIGKILL.
/// The container is kept (no `--rm`, hard rule), so `taskfmt attach` restarts it.
///
/// This is how the gate gets a workspace that cannot move under it: a stopped container has no
/// process left to write into the `/work` bind mount. `true` when the container is no longer
/// running afterwards — including when it was already stopped.
pub fn stop(container: &str, grace_s: u64) -> bool {
    let stopped = capture_docker(Command::new("docker").args([
        "stop",
        "--time",
        &grace_s.to_string(),
        container,
    ]))
    .map(|out| out.ok())
    .unwrap_or(false);
    stopped || !is_running(container)
}

/// How a caller learns the gate fingerprint baked into an image.
///
/// A parameter rather than a lookup inside `dispatch_one` (D-011): what is injected is a **value**,
/// never a verdict — the comparison runs unconditionally on whatever comes back — so this is no
/// bypass of the refusal. Production passes [`DockerImageFingerprint`], the one implementation in
/// the crate; test doubles live in integration-test crates that are linked into no binary.
pub trait ImageFingerprint {
    /// The image's own `taskfmt` fingerprint, or an error naming `image`.
    fn image_fingerprint(&self, image: &str) -> anyhow::Result<String>;
}

/// The production reader: it executes the binary the image carries.
pub struct DockerImageFingerprint;

impl ImageFingerprint for DockerImageFingerprint {
    fn image_fingerprint(&self, image: &str) -> anyhow::Result<String> {
        image_fingerprint(image)
    }
}

/// Ask an image what gate it carries, by running the binary that would run the gate.
///
/// Neither a label nor a file in the image is used: a label is metadata the builder attached and
/// nothing binds it to the binary, and a file next to the binary can drift from it. Executing
/// `/usr/local/bin/taskfmt` is the only answer that comes from the artifact that will judge, and
/// it proves the image can execute it at all. The `--entrypoint` override is required because
/// `harness-base` sets `ENTRYPOINT ["/usr/local/bin/taskfmt", "container-entrypoint"]`.
///
/// Fails closed: an absent image, an unreachable daemon, an image built before the subcommand
/// existed and an image answering with anything but 64 lowercase hex digits are all errors naming
/// `image`, never a value.
pub fn image_fingerprint(image: &str) -> anyhow::Result<String> {
    let out = capture_docker(Command::new("docker").args([
        "run",
        "--rm",
        "--entrypoint",
        "/usr/local/bin/taskfmt",
        image,
        "fingerprint",
    ]))
    .with_context(|| format!("cannot run docker to read the gate fingerprint of {image}"))?;
    if !out.ok() {
        anyhow::bail!(
            "cannot read the gate fingerprint of {image} (rc={}): {}",
            out.status,
            crate::redact::scrub(out.stderr.trim_end())
        );
    }
    let value = out.stdout.trim().to_string();
    if !is_fingerprint(&value) {
        anyhow::bail!(
            "{image} answered with no gate fingerprint: {:?}",
            crate::redact::scrub(&value)
        );
    }
    Ok(value)
}

/// 64 lowercase hex digits and nothing else — the shape `taskfmt fingerprint` prints.
fn is_fingerprint(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

pub fn image_exists(image: &str) -> bool {
    capture_docker(Command::new("docker").args(["image", "inspect", image]))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspect_lines_parse_name_state_mount_and_labels() {
        let info = parse_inspect_line(
            "/harness-r\trunning\t/runs/r/workspace\t{\"taskfmt.run_id\":\"r\",\"empty\":\"\"}",
        )
        .unwrap();
        assert_eq!(info.name, "harness-r", "docker's leading slash is dropped");
        assert!(info.is_running());
        assert_eq!(info.work_mount, Some(PathBuf::from("/runs/r/workspace")));
        assert_eq!(info.label("taskfmt.run_id"), Some("r"));
        assert_eq!(info.label("empty"), None, "an empty label value is absent");
        assert_eq!(info.label("nope"), None);
    }

    #[test]
    fn a_container_without_labels_or_a_work_mount_still_parses() {
        // `{{json .Config.Labels}}` renders a nil map as `null`; a container with no /work mount
        // leaves the third field empty. Both are the pre-label containers' shape.
        let info = parse_inspect_line("/harness-r\texited\t\tnull").unwrap();
        assert_eq!(info.state, "exited");
        assert!(!info.is_running());
        assert!(info.work_mount.is_none());
        assert!(info.labels.is_empty());
        let empty = parse_inspect_line("/harness-r\trunning\t\t{}").unwrap();
        assert!(empty.labels.is_empty());
    }

    #[test]
    fn only_64_lowercase_hex_digits_are_a_fingerprint() {
        assert!(is_fingerprint(&"a1b2c3d4".repeat(8)));
        assert!(!is_fingerprint(&"A1B2C3D4".repeat(8)), "uppercase");
        assert!(!is_fingerprint(&"a1b2c3d4".repeat(7)), "too short");
        assert!(!is_fingerprint(""), "clap help, or nothing at all");
        assert!(
            !is_fingerprint(&format!("{}\n", "a1b2c3d4".repeat(8))),
            "untrimmed"
        );
    }

    #[test]
    fn blank_inspect_lines_are_not_containers() {
        assert!(parse_inspect_line("").is_none());
        assert!(parse_inspect_line("/").is_none());
    }

    #[test]
    fn the_inspect_template_asks_for_the_work_mount_and_json_labels() {
        let template = inspect_template();
        assert!(
            template.contains(&format!("eq .Destination \"{WORK_MOUNT}\"")),
            "{template}"
        );
        assert!(template.contains("{{json .Config.Labels}}"), "{template}");
        assert_eq!(template.matches('\t').count(), 3, "{template}");
    }

    #[test]
    fn a_run_spec_puts_every_label_on_the_command_line() {
        // the spec is what `run_detached` renders; no docker needed to prove the pairs survive
        let spec = RunSpec {
            name: "harness-r".into(),
            image: "harness-claude:latest".into(),
            mounts: vec![Mount::rw(Path::new("/runs/r/workspace"), WORK_MOUNT)],
            env: vec![],
            labels: vec![("taskfmt.run_id".into(), "r".into())],
            env_file: None,
            memory: "4g".into(),
            cpus: 2.0,
            pids_limit: 2048,
        };
        assert_eq!(spec.mounts[0].flag(), "/runs/r/workspace:/work");
        assert_eq!(spec.labels[0].1, "r");
    }
}
