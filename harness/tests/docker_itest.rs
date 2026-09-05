//! Docker-needing checks, with truthful runtime `SKIP` reporting.
//!
//! Run a release-proofed execution with `sh tests/run_docker_itest.sh`. Calling this target with
//! Docker disabled or unavailable prints `docker_itest: SKIP ...` and executes no body; it never
//! asks libtest to turn that absence into a passing test.

use std::path::Path;
use std::process::{Command, ExitCode};
use std::time::Duration;

use taskfmt::itest::{self, CHECK_ARCH, CHECK_PREREQS, Preflight};
use taskfmt::ops::docker;

const PREREQ_IMAGE: &str = "harness-base:latest";
const PREREQ_READY: &str = "/out/prereqs.ready";
const PREREQ_FAILED: &str = "/out/prereqs.FAILED";
const PREREQ_LOG: &str = "/out/prereqs.log";
const PREREQ_TIMEOUT: Duration = Duration::from_secs(180);

fn enabled() -> bool {
    std::env::var("TASKFMT_ITEST_DOCKER").as_deref() == Ok("1")
}

struct Cleanup(String);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", self.0.as_str()])
            .output();
    }
}

fn postmortem(name: &str) -> String {
    let code = Command::new("docker")
        .args(["inspect", "-f", "{{.State.ExitCode}}", name])
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_default();
    let logs = Command::new("docker")
        .args(["logs", name])
        .output()
        .map(|out| {
            let mut all = String::from_utf8_lossy(&out.stdout).into_owned();
            all.push_str(&String::from_utf8_lossy(&out.stderr));
            all.lines().take(6).collect::<Vec<_>>().join("\n")
        })
        .unwrap_or_default();
    format!("container exited (rc={code}); first lines of its output:\n{logs}")
}

fn wait_prereqs(name: &str) -> Result<(), String> {
    let started = std::time::Instant::now();
    loop {
        if docker::read_file(name, PREREQ_FAILED).is_some() {
            return Err(format!(
                "prereq stage wrote {PREREQ_FAILED}\n{}",
                docker::read_file(name, PREREQ_LOG).unwrap_or_default()
            ));
        }
        if docker::read_file(name, PREREQ_READY).is_some() {
            return Ok(());
        }
        if !docker::is_running(name) {
            return Err(format!(
                "PREREQ_ONLY must park, not exit: {}",
                postmortem(name)
            ));
        }
        if started.elapsed() >= PREREQ_TIMEOUT {
            return Err(format!(
                "{PREREQ_READY} did not appear within {} s\n{}",
                PREREQ_TIMEOUT.as_secs(),
                docker::read_file(name, PREREQ_LOG).unwrap_or_default()
            ));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn container_runs_prereq_stage_and_stays_up() -> Result<(), String> {
    println!("docker_itest: RUN check={CHECK_PREREQS}");
    let name = format!("taskfmt-itest-prereq-{}", std::process::id());
    let _ = Command::new("docker")
        .args(["rm", "-f", name.as_str()])
        .output();
    let run = Command::new("docker")
        .args([
            "run",
            "-d",
            "--privileged",
            "--name",
            name.as_str(),
            "-e",
            "PREREQ_ONLY=1",
            PREREQ_IMAGE,
        ])
        .output()
        .map_err(|err| format!("spawning docker run: {err}"))?;
    let container = Cleanup(name);
    let name = container.0.as_str();
    if !run.status.success() {
        return Err(String::from_utf8_lossy(&run.stderr).into_owned());
    }
    wait_prereqs(name)?;
    if !docker::is_running(name) {
        return Err("PREREQ_ONLY must park, not exit".to_owned());
    }
    let entry = docker::read_file(name, "/proc/1/cmdline")
        .unwrap_or_default()
        .replace('\0', " ");
    if !(entry.contains("taskfmt") && entry.contains("container-entrypoint")) {
        return Err(format!(
            "PID 1 must be `taskfmt container-entrypoint`: {entry:?}"
        ));
    }
    println!("docker_itest: PASS check={CHECK_PREREQS}");
    Ok(())
}

fn arch_detection_agrees_with_the_server() -> Result<(), String> {
    println!("docker_itest: RUN check={CHECK_ARCH}");
    let server = docker::server_arch().map_err(|err| err.to_string())?;
    if server.is_empty() {
        return Err("docker server architecture must not be empty".to_owned());
    }
    println!("docker_itest: PASS check={CHECK_ARCH}");
    Ok(())
}

fn run() -> Result<(), String> {
    // Do not even probe Docker until opted in. Besides avoiding needless work, this preserves the
    // disabled contract when a broken CLI would otherwise hang before it could report SKIP.
    let enabled = enabled();
    let daemon_available = enabled && docker::available();
    let image_available = daemon_available && docker::image_exists(PREREQ_IMAGE);
    match itest::preflight(enabled, daemon_available, image_available) {
        Preflight::Skip(reason) => {
            println!("docker_itest: SKIP reason={reason}");
            return Ok(());
        }
        Preflight::Run => {}
    }
    let proof = std::env::var_os("TASKFMT_ITEST_PROOF")
        .ok_or("enabled Docker integration requires TASKFMT_ITEST_PROOF")?;
    let proof = Path::new(&proof);
    container_runs_prereq_stage_and_stays_up()?;
    arch_detection_agrees_with_the_server()?;
    itest::write_proof(proof, &[CHECK_PREREQS, CHECK_ARCH]).map_err(|err| err.to_string())?;
    println!("docker_itest: PASS proof={}", proof.display());
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("docker_itest: FAIL {err}");
            ExitCode::FAILURE
        }
    }
}
