//! `taskfmt container-entrypoint` — container PID 1 (root): inner dockerd (DinD, vfs) → prereqs →
//! agent. HARD RULE: never exit on prereq failure — park so the operator can re-attach and debug
//! the live container. Markers in /out: prereqs.json, prereqs.ready, prereqs.FAILED, prereqs.log,
//! dockerd.log.

use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::Context;

use crate::ops::signals;
use crate::redact;

const PG_CONTAINER: &str = "prereq-postgres";
const PG_IMAGE: &str = "postgres:16-alpine";
const PG_USER: &str = "pgtui";
const PG_PASSWORD: &str = "pgtui";
const PG_DB: &str = "pgtui";
const PRELOAD_TAR: &str = "/opt/preload/postgres.tar";
/// Where the claude image stages the plugin install at build time (see images/claude/Dockerfile).
const CLAUDE_PLUGIN_SEED: &str = "/opt/claude-plugin-seed";
const OUT: &str = "/out";
const PARK: Duration = Duration::from_secs(86400);

pub fn run() -> anyhow::Result<i32> {
    let _flags = signals::install_terminate_flag();

    // (a) inner Docker daemon. A docker stop that escalates to SIGKILL leaves a stale
    //     /var/run/docker.pid behind which blocks the daemon on the next boot — drop it.
    let _ = std::fs::remove_file("/var/run/docker.pid");
    let dockerd_log =
        std::fs::File::create(format!("{OUT}/dockerd.log")).context("creating /out/dockerd.log")?;
    let _dockerd = Command::new("dockerd")
        .args([
            "--data-root",
            "/var/lib/docker",
            "--storage-driver",
            &std::env::var("DOCKERD_STORAGE_DRIVER").unwrap_or_else(|_| "vfs".into()),
            "--host",
            "unix:///var/run/docker.sock",
        ])
        .stdout(Stdio::from(dockerd_log.try_clone()?))
        .stderr(Stdio::from(dockerd_log))
        .spawn()
        .context("spawning dockerd")?;
    wait_dockerd(Duration::from_secs(60));
    let _ = Command::new("chmod")
        .args(["666", "/var/run/docker.sock"])
        .status();

    // codex image: seed $CODEX_HOME/config.toml from the baked copy (a fresh run mounts an empty
    // dir) and write auth.json before the agent starts (OPENAI_API_KEY comes through --env-file).
    let codex_home = std::env::var("CODEX_HOME").unwrap_or_default();
    if !codex_home.is_empty() {
        let _ = std::fs::create_dir_all(&codex_home);
        if Path::new("/etc/codex-config.toml").is_file()
            && !Path::new(&codex_home).join("config.toml").is_file()
        {
            let _ = std::fs::copy(
                "/etc/codex-config.toml",
                Path::new(&codex_home).join("config.toml"),
            );
        }
        let _ = chown_agent(&codex_home);
        // the key arrives through --env-file and is piped into codex-login's stdin: it never
        // appears in an argv, a log line, or an artifact
        let key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
        if !key.is_empty() && !Path::new(&codex_home).join("auth.json").is_file() {
            let status = Command::new("gosu")
                .args(["agent", "taskfmt", "codex-login"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()
                .and_then(|mut child| {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(key.as_bytes());
                    }
                    child.wait()
                })
                .context("codex login")?;
            if !status.success() {
                redact::eemit("entrypoint: codex login failed — check OPENAI_API_KEY");
            }
        }
    }
    // claude image: seed $CLAUDE_CONFIG_DIR/plugins from the baked /opt/claude-plugin-seed (a
    // fresh run bind-mounts an empty /agent-home, which masks whatever the image installed
    // there). Idempotent: an existing plugins/ tree belongs to this run's agent, keep it.
    let claude_home = std::env::var("CLAUDE_CONFIG_DIR").unwrap_or_default();
    if !claude_home.is_empty()
        && seed_claude_plugins(Path::new(CLAUDE_PLUGIN_SEED), Path::new(&claude_home))?
    {
        let _ = chown_agent_recursive(&Path::new(&claude_home).join("plugins").to_string_lossy());
    }
    for dir in ["/work", "/out", "/agent-home"] {
        let _ = chown_agent(dir);
    }

    // (b) prerequisites. Stale markers from a previous boot (docker stop/start) must not survive.
    let _ = std::fs::remove_file(format!("{OUT}/prereqs.ready"));
    let _ = std::fs::remove_file(format!("{OUT}/prereqs.FAILED"));
    match prereqs() {
        Ok(json) => {
            let _ = std::fs::File::create(format!("{OUT}/prereqs.ready"));
            redact::write_scrubbed(&Path::new(OUT).join("prereqs.json"), json.as_bytes()).ok();
            redact::emit(&format!("entrypoint: prereqs ok — {}", json.trim()));
        }
        Err(err) => {
            let _ = std::fs::File::create(format!("{OUT}/prereqs.FAILED"));
            let error = read_tail(&format!("{OUT}/prereqs.log"), 20, 2000);
            let body = serde_json::json!({"ok": false, "error": error}).to_string();
            redact::write_scrubbed(&Path::new(OUT).join("prereqs.json"), body.as_bytes()).ok();
            redact::eemit(&format!(
                "entrypoint: prerequisites failed ({err:#}) — parking"
            ));
            redact::eemit("entrypoint: inspect /out/prereqs.log inside the container");
            park();
        }
    }

    // (d) test mode: prerequisites only (prereqs.ready written above), no agent
    if std::env::var("PREREQ_ONLY").as_deref() == Ok("1") {
        redact::eemit("entrypoint: PREREQ_ONLY=1 — prerequisites done, parking without agent");
        park();
    }

    // (c) agent supervision as user `agent`; TERM/INT handled there (graceful herdr server stop)
    let err = Command::new("gosu")
        .args(["agent", "taskfmt", "agent-launch"])
        .exec();
    Err(err).context("exec gosu agent taskfmt agent-launch")
}

/// `taskfmt prereqs` — the prereq stage on its own (also usable interactively inside the container).
pub fn prereqs_only() -> anyhow::Result<i32> {
    match prereqs() {
        Ok(json) => {
            let _ = std::fs::File::create(format!("{OUT}/prereqs.ready"));
            redact::write_scrubbed(&Path::new(OUT).join("prereqs.json"), json.as_bytes()).ok();
            redact::emit(&format!("prereqs ok — {}", json.trim()));
            Ok(0)
        }
        Err(err) => {
            let _ = std::fs::File::create(format!("{OUT}/prereqs.FAILED"));
            let body = serde_json::json!({"ok": false, "error": err.to_string()}).to_string();
            redact::write_scrubbed(&Path::new(OUT).join("prereqs.json"), body.as_bytes()).ok();
            redact::eemit(&format!("prereqs FAILED: {err:#}"));
            Ok(1)
        }
    }
}

fn wait_dockerd(timeout: Duration) {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if Command::new("docker")
            .arg("info")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            return;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    redact::eemit(
        "entrypoint: dockerd not ready after 60s — see /out/dockerd.log; prerequisites will fail",
    );
}

/// postgres 16 (inner docker) + seed restore. Success markers: /out/prereqs.ready + prereqs.json.
/// No seed file at all is fine: postgres up = prerequisite met.
fn prereqs() -> anyhow::Result<String> {
    let started = Instant::now();
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(format!("{OUT}/prereqs.log"))
        .context("opening /out/prereqs.log")?;
    let step = |message: &str| -> anyhow::Result<()> {
        let mut handle = log.try_clone().context("cloning prereqs.log handle")?;
        writeln!(handle, "=== prereqs {message}")?;
        Ok(())
    };
    step(&format!("start {}", crate::config::timestamp_rfc3339()))?;

    // 1. baked image tarball (built by `taskfmt preload` on the host)
    if !Path::new(PRELOAD_TAR).is_file() {
        anyhow::bail!("{PRELOAD_TAR} missing — run `taskfmt preload` first");
    }
    crate::ops::check(
        Command::new("docker").args(["load", "-i", PRELOAD_TAR]),
        &format!("docker load {PRELOAD_TAR}"),
    )?;

    // 2. postgres on loopback only; idempotent on container restart
    let _ = Command::new("docker")
        .args(["rm", "-f", PG_CONTAINER])
        .stdout(Stdio::null())
        .status();
    crate::ops::check(
        Command::new("docker")
            .args([
                "run",
                "-d",
                "--name",
                PG_CONTAINER,
                "-p",
                "127.0.0.1:5432:5432",
            ])
            .args([
                "-e",
                &format!("POSTGRES_USER={PG_USER}"),
                "-e",
                &format!("POSTGRES_PASSWORD={PG_PASSWORD}"),
                "-e",
                &format!("POSTGRES_DB={PG_DB}"),
            ])
            .arg(PG_IMAGE),
        "docker run prereq-postgres",
    )?;
    let ready = wait_pg(Duration::from_secs(60));
    if !ready {
        anyhow::bail!("postgres never became ready on 127.0.0.1:5432 (see /out/prereqs.log)");
    }

    // 3. seeds: /work/**/tests/fixtures/seed.sql (sorted), then /seed/*.sql (sorted) — ON_ERROR_STOP
    let mut seeds = find_sorted("/work", |path| {
        path.file_name().is_some_and(|name| name == "seed.sql")
            && path.to_string_lossy().contains("/tests/fixtures/")
    });
    seeds.extend(find_sorted("/seed", |path| {
        path.extension().is_some_and(|ext| ext == "sql")
    }));
    if seeds.is_empty() {
        step("WARN: no seed files found — continuing (postgres up = prerequisite met)")?;
    }
    let mut restored: Vec<String> = Vec::new();
    for seed in &seeds {
        step(&format!("restore seed {}", seed.display()))?;
        let file = std::fs::File::open(seed)
            .with_context(|| format!("opening seed {}", seed.display()))?;
        let status = Command::new("docker")
            .args([
                "exec",
                "-i",
                PG_CONTAINER,
                "psql",
                "-U",
                PG_USER,
                "-d",
                PG_DB,
                "-v",
                "ON_ERROR_STOP=1",
            ])
            .stdin(Stdio::from(file))
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log.try_clone()?))
            .status()
            .context("docker exec psql")?;
        if !status.success() {
            anyhow::bail!("seed restore failed: {} (exit {status})", seed.display());
        }
        restored.push(seed.display().to_string());
    }

    step(&format!(
        "ok: {PG_IMAGE} up, {} seed(s) restored ({} s)",
        restored.len(),
        started.elapsed().as_secs()
    ))?;
    Ok(serde_json::json!({
        "ok": true,
        "postgres_dsn": format!("postgres://{PG_USER}:********@127.0.0.1:5432/{PG_DB}"),
        "postgres_container": PG_CONTAINER,
        "image": PG_IMAGE,
        "seeds": restored,
        "finished": crate::config::timestamp_rfc3339(),
    })
    .to_string())
}

fn wait_pg(timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if Command::new("pg_isready")
            .args(["-h", "127.0.0.1", "-p", "5432", "-U", PG_USER])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            return true;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    false
}

/// Sorted file list under `root` (missing root = empty) filtered by `keep`.
fn find_sorted(root: &str, keep: impl Fn(&Path) -> bool) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let Ok(entry) = entry else { continue };
        if entry.file_type().is_file() && keep(entry.path()) {
            found.push(entry.path().to_path_buf());
        }
    }
    found.sort();
    found
}

fn read_tail(path: &str, lines: usize, max_chars: usize) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let tail: Vec<&str> = text.lines().rev().take(lines).collect();
    let joined: String = tail.into_iter().rev().collect::<Vec<_>>().join(" ");
    joined.chars().take(max_chars).collect()
}

/// Stay alive, debuggable; SIGTERM → clean exit 0. Never returns.
fn park() -> ! {
    loop {
        if signals::sleep_until_terminate(PARK) {
            std::process::exit(0);
        }
    }
}

/// `chown agent` on a path (best effort; the container image owns the `agent` uid).
fn chown_agent(path: &str) -> anyhow::Result<()> {
    let status = Command::new("chown").args(["agent", path]).status()?;
    if !status.success() {
        anyhow::bail!("chown agent {path} failed");
    }
    Ok(())
}

/// `chown -R agent` on a path (best effort; for trees copied into the agent's home as root).
fn chown_agent_recursive(path: &str) -> anyhow::Result<()> {
    let status = Command::new("chown").args(["-R", "agent", path]).status()?;
    if !status.success() {
        anyhow::bail!("chown -R agent {path} failed");
    }
    Ok(())
}

/// Copy the baked plugin tree (`<seed_dir>/plugins`) into `<agent_home>/plugins`. Returns true
/// when anything was copied. Idempotent: a missing seed or an already-present plugins/ tree is a
/// no-op, so an existing per-run config is never clobbered.
fn seed_claude_plugins(seed_dir: &Path, agent_home: &Path) -> anyhow::Result<bool> {
    let src = seed_dir.join("plugins");
    let dst = agent_home.join("plugins");
    if !src.is_dir() || dst.exists() {
        return Ok(false);
    }
    crate::ops::copy_tree(&src, &dst)
        .with_context(|| format!("copying {} to {}", src.display(), dst.display()))?;
    Ok(true)
}

/// `taskfmt codex-login` — reads the API key on stdin (never argv) and writes codex auth.json.
/// Invoked by the entrypoint through gosu so the secret never crosses a process argument.
pub fn codex_login() -> anyhow::Result<i32> {
    use std::io::Read;
    use std::os::unix::fs::PermissionsExt;
    let mut key = String::new();
    std::io::stdin()
        .read_to_string(&mut key)
        .context("reading the API key on stdin")?;
    let key = key.trim().to_string();
    if key.is_empty() {
        anyhow::bail!("empty API key on stdin");
    }
    redact::register(key.clone());
    let home = std::env::var("CODEX_HOME").unwrap_or_else(|_| "/agent-home".to_string());
    std::fs::create_dir_all(&home).context("creating CODEX_HOME")?;
    let auth = Path::new(&home).join("auth.json");
    // the credential's only sanctioned resting place: 0600, never echoed, never in an artifact
    std::fs::write(&auth, format!("{{\"OPENAI_API_KEY\":\"{key}\"}}\n"))
        .context("writing auth.json")?;
    std::fs::set_permissions(&auth, std::fs::Permissions::from_mode(0o600))
        .context("chmod 600 auth.json")?;
    let _ = chown_agent(&home);
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake baked seed: `<seed>/plugins/<tree…>`.
    fn fake_seed(seed: &Path) {
        let cache = seed.join("plugins/cache/rust-analyzer-lsp");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(cache.join("plugin.json"), "{}").unwrap();
        std::fs::write(seed.join("plugins/installed_plugins.json"), "{}").unwrap();
    }

    #[test]
    fn plugin_seed_copies_the_tree_once_and_never_twice() {
        let dir = tempfile::tempdir().unwrap();
        let seed = dir.path().join("seed");
        let home = dir.path().join("agent-home");
        std::fs::create_dir_all(&home).unwrap();
        fake_seed(&seed);

        assert!(
            seed_claude_plugins(&seed, &home).unwrap(),
            "first boot copies"
        );
        assert!(
            home.join("plugins/cache/rust-analyzer-lsp/plugin.json")
                .is_file(),
            "the whole tree came along"
        );

        // a file the agent wrote inside its own plugins/ tree must survive a re-boot
        std::fs::write(home.join("plugins/marker"), "mine").unwrap();
        assert!(
            !seed_claude_plugins(&seed, &home).unwrap(),
            "second boot is a no-op"
        );
        assert_eq!(
            std::fs::read_to_string(home.join("plugins/marker")).unwrap(),
            "mine"
        );
    }

    #[test]
    fn plugin_seed_is_a_noop_without_a_baked_seed() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("agent-home");
        assert!(!seed_claude_plugins(&dir.path().join("missing"), &home).unwrap());
        assert!(!home.join("plugins").exists());
    }

    #[test]
    fn plugin_seed_preserves_existing_agent_home_files() {
        let dir = tempfile::tempdir().unwrap();
        let seed = dir.path().join("seed");
        let home = dir.path().join("agent-home");
        std::fs::create_dir_all(&home).unwrap();
        fake_seed(&seed);
        std::fs::write(home.join("settings.json"), "{}").unwrap();

        assert!(seed_claude_plugins(&seed, &home).unwrap());
        assert!(
            home.join("settings.json").is_file(),
            "the seeded copy touches only plugins/"
        );
    }
}
