//! The only monitor boundary allowed to launch `taskfmt`.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::watch;

use crate::config::ExperimentConfig;
use crate::runstate::Manifest;

#[derive(Clone, Debug)]
pub struct AdapterConfig {
    pub taskfmt: PathBuf,
    pub experiment_config: PathBuf,
    pub repo: String,
    pub agent: Option<String>,
}

impl AdapterConfig {
    /// Validate operator configuration before exposing execution actions.
    pub fn validate(&self) -> anyhow::Result<()> {
        if !self.taskfmt.is_file() || self.repo.is_empty() || self.repo.starts_with('-') {
            bail!("execution requires a taskfmt executable and an explicit repository");
        }
        let resolved = ExperimentConfig::load_resolved(&self.experiment_config)?;
        resolved.cfg.profile(
            self.agent
                .as_deref()
                .unwrap_or_else(|| resolved.cfg.default_profile()),
        )?;
        Ok(())
    }

    pub async fn execute(
        &self,
        package: PathBuf,
        artifacts: PathBuf,
        mut cancel: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Manifest>> {
        let config = self.clone();
        let prepare_root = artifacts.clone();
        let private_config =
            tokio::task::spawn_blocking(move || config.prepare(&prepare_root)).await??;
        let job = SupervisorJob {
            taskfmt: self.taskfmt.clone(),
            private_config,
            package,
            repo: self.repo.clone(),
            agent: self.agent.clone(),
        };
        super::atomic_json::write(&artifacts.join("supervisor.json"), &job)?;
        if *cancel.borrow() {
            bail!("execution cancelled before launch");
        }
        let mut supervisor = Command::new(supervisor_path()?);
        supervisor
            .arg(&artifacts)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(unix)]
        supervisor.process_group(0);
        let mut child = supervisor
            .spawn()
            .context("cannot launch task-monitor-supervisor; build/install all harness binaries")?;
        let mut control = child
            .stdin
            .take()
            .context("missing supervisor control pipe")?;
        let mut output = BufReader::new(
            child
                .stdout
                .take()
                .context("missing supervisor handshake")?,
        );
        let mut ready = String::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            output.read_line(&mut ready),
        )
        .await??;
        if ready.trim() != "READY" {
            bail!("supervisor failed before ownership handshake");
        }
        control.write_all(b"GO\n").await?;
        let result = tokio::select! {
            result=child.wait()=>result.context("cannot wait for supervisor"),
            _=cancel.changed()=>{
                drop(control);
                // EOF owns cancellation. Do not kill the supervisor before it cleans Docker.
                child.wait().await.context("cannot wait for cancelled supervisor")
            }
        };
        // A failed launch can still have created a container. Stop only containers labelled
        // with this execution's private configuration, including pre-manifest failures.
        cleanup(&artifacts).await?;
        let status = result?;
        let manifest = tokio::task::spawn_blocking(move || find_manifest(&artifacts)).await??;
        if !status.success() && manifest.as_ref().is_none_or(|m| !verified(m)) {
            bail!("taskfmt execution failed; inspect the operator adapter log");
        }
        Ok(manifest)
    }

    fn prepare(&self, artifacts: &Path) -> anyhow::Result<PathBuf> {
        self.validate()?;
        let resolved = ExperimentConfig::load_resolved(&self.experiment_config)?;
        let mut cfg = resolved.cfg.clone();
        cfg.paths.tasks_dir = resolved.tasks_dir().to_string_lossy().into_owned();
        cfg.paths.seed_dir = resolved.seed_dir().to_string_lossy().into_owned();
        cfg.paths.template_dir = resolved.template_dir().to_string_lossy().into_owned();
        cfg.paths.runs_dir = artifacts.to_string_lossy().into_owned();
        let path = artifacts.join("experiment.toml");
        super::atomic_json::write_bytes(&path, toml::to_string(&cfg)?.as_bytes())?;
        Ok(path)
    }
}

fn supervisor_path() -> anyhow::Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let directory = executable.parent().context("executable has no parent")?;
    let sibling = directory.join("task-monitor-supervisor");
    if sibling.is_file() {
        return Ok(sibling);
    }
    // libtest executables live in target/debug/deps; the real supervisor remains adjacent
    // to the application binaries, never a browser- or PATH-selected command.
    if directory.file_name().is_some_and(|name| name == "deps") {
        let sibling = directory
            .parent()
            .context("test binary has no target directory")?
            .join("task-monitor-supervisor");
        if sibling.is_file() {
            return Ok(sibling);
        }
    }
    bail!("task-monitor-supervisor must be installed beside task-monitor")
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SupervisorJob {
    taskfmt: PathBuf,
    private_config: PathBuf,
    package: PathBuf,
    repo: String,
    agent: Option<String>,
}

/// Pipe-coupled process owner. READY is sent only after acquiring the artifact lease;
/// GO can therefore never authorize a child that restart recovery mistakes for absent.
pub async fn supervise(artifacts: PathBuf) -> anyhow::Result<()> {
    let artifacts = artifacts.canonicalize()?;
    let _lease = crate::monitor::RunLock::acquire(&artifacts)?;
    let job: SupervisorJob =
        serde_json::from_slice(&std::fs::read(artifacts.join("supervisor.json"))?)?;
    use std::io::Write;
    println!("READY");
    std::io::stdout().flush()?;
    let mut input = BufReader::new(tokio::io::stdin());
    let mut go = String::new();
    input.read_line(&mut go).await?;
    if go != "GO\n" {
        return Ok(());
    }
    let log = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(artifacts.join("adapter.log"))?;
    let mut command = Command::new(job.taskfmt);
    command
        .arg("--config")
        .arg(job.private_config)
        .arg("--auto")
        .args(["run", "--task"])
        .arg(job.package)
        .arg("--repo")
        .arg(job.repo)
        .arg("--wait")
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .kill_on_drop(true);
    if let Some(agent) = job.agent {
        command.arg("--agent").arg(agent);
    }
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command.spawn()?;
    let pid = child.id();
    let mut line = String::new();
    let result = tokio::select! {
        result=child.wait()=>result,
        _=input.read_line(&mut line)=>{
            #[cfg(unix)]
            if let Some(pid)=pid {
                let _=Command::new("kill").args(["-KILL","--",&format!("-{pid}")]).status().await;
            }
            let _=child.kill().await;
            child.wait().await
        }
    };
    cleanup(&artifacts).await?;
    anyhow::ensure!(result?.success(), "taskfmt exited unsuccessfully");
    Ok(())
}

pub(crate) async fn await_supervisor(root: &Path) -> anyhow::Result<crate::monitor::RunLock> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        match crate::monitor::RunLock::acquire(root) {
            Ok(lease) => return Ok(lease),
            Err(error) if tokio::time::Instant::now() >= deadline => {
                return Err(error.context("supervisor cleanup is still active"));
            }
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }
}

pub(crate) fn verified(manifest: &Manifest) -> bool {
    manifest
        .gate
        .as_ref()
        .is_some_and(|gate| gate.promotable() && gate.exit == 0 && gate.last_line == "DONE")
}

/// Artifact discovery never follows links supplied by a run or returns a second run.
pub(crate) fn find_manifest(root: &Path) -> anyhow::Result<Option<Manifest>> {
    anyhow::ensure!(
        !std::fs::symlink_metadata(root)?.file_type().is_symlink(),
        "execution root cannot be a symlink"
    );
    let canonical = root.canonicalize()?;
    let mut found = None;
    for entry in std::fs::read_dir(&canonical)? {
        let entry = entry?;
        if entry.file_type()?.is_symlink() {
            bail!("symlink in execution artifacts");
        }
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path().join("manifest.json");
        if !path.exists() {
            continue;
        }
        if std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
            bail!("symlink run manifest");
        }
        let manifest = Manifest::load(&entry.path())?;
        if Path::new(&manifest.run_dir).canonicalize()? != entry.path().canonicalize()? {
            bail!("run manifest points outside its execution");
        }
        if found.replace(manifest).is_some() {
            bail!("multiple taskfmt manifests in one execution");
        }
    }
    Ok(found)
}

pub(crate) async fn cleanup(root: &Path) -> anyhow::Result<()> {
    let config = root.join("experiment.toml");
    if !config.exists() {
        return Ok(());
    }
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        Command::new("docker")
            .args(["ps", "-q", "--filter"])
            .arg(format!("label=taskfmt.manifest={}", config.display()))
            .kill_on_drop(true)
            .output(),
    )
    .await;
    // Containers may exist before the first manifest is durable. Failed inspection is
    // therefore never proof of cleanup, even when no manifest can be found.
    let output = match output {
        Ok(Ok(output)) if output.status.success() => output,
        _ => bail!("cannot establish container cleanup"),
    };
    for id in String::from_utf8(output.stdout)?.lines() {
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            bail!("invalid Docker container identity");
        }
        let status = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            Command::new("docker")
                .args(["stop", "--time", "5", id])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .status(),
        )
        .await??;
        if !status.success() {
            bail!("cannot stop owned container");
        }
    }
    Ok(())
}
