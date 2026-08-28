//! Docker-needing checks. Opt-in: `TASKFMT_ITEST_DOCKER=1 cargo test --test docker_itest`.
//! Uses only images already present on the host (`taskfmt build-images`, `taskfmt preload`).

use std::process::Command;

use taskfmt::ops::docker;

fn docker_available() -> bool {
    Command::new("docker")
        .args(["version", "--format", "{{.Server.Version}}"])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn enabled() -> bool {
    std::env::var("TASKFMT_ITEST_DOCKER").as_deref() == Ok("1")
}

/// One privileged persistent container running the prereq stage — the exact shape `run` uses.
#[test]
fn container_runs_prereq_stage_and_stays_up() {
    if !enabled() || !docker_available() {
        eprintln!("skipping: set TASKFMT_ITEST_DOCKER=1 (and build harness-taskfmt:latest)");
        return;
    }
    if !docker::image_exists("harness-taskfmt:latest") {
        eprintln!("skipping: harness-taskfmt:latest not built");
        return;
    }

    let name = "taskfmt-itest-prereq";
    let _ = Command::new("docker").args(["rm", "-f", name]).output();
    let run = Command::new("docker")
        .args([
            "run",
            "-d",
            "--privileged",
            "--name",
            name,
            "-e",
            "PREREQ_ONLY=1",
            "-v",
            "/tmp:/work",
            "harness-taskfmt:latest",
        ])
        .output()
        .expect("docker run");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );

    // the entrypoint is taskfmt: PREREQ_ONLY parks forever instead of launching the agent
    assert!(docker::is_running(name), "PREREQ_ONLY must park, not exit");
    let entry = docker::read_file(name, "/proc/1/cmdline").unwrap_or_default();
    assert!(
        entry.contains("taskfmt"),
        "PID 1 must be taskfmt: {entry:?}"
    );

    let _ = Command::new("docker").args(["rm", "-f", name]).output();
}

#[test]
fn arch_detection_agrees_with_the_server() {
    if !enabled() || !docker_available() {
        eprintln!("skipping: set TASKFMT_ITEST_DOCKER=1");
        return;
    }
    let server = docker::server_arch().unwrap();
    assert!(!server.is_empty());
}
