//! Docker-needing checks. Opt-in: `TASKFMT_ITEST_DOCKER=1 cargo test --test docker_itest`.
//! Uses only images already present on the host (`taskfmt build-images`, `taskfmt preload`).
//!
//! Known deviation, recorded rather than fixed: with the switch unset both tests return early and
//! the run still reports "2 passed", so a skipped run is indistinguishable from a covered one. The
//! attribute that would make the skip visible in the test report is itself a forbidden pattern
//! under `harness/tests` (no test may be silenced to make the suite green), so the switch has to be
//! turned on by whoever runs the gate.

use std::process::Command;
use std::time::Duration;

use taskfmt::ops::docker;

/// The image whose `ENTRYPOINT` is `taskfmt container-entrypoint` (`harness/images/base/Dockerfile`).
/// `harness-taskfmt:latest` is a different image with a bare `taskfmt` entrypoint and no `CMD`; it
/// never reaches `container-entrypoint`, so `PREREQ_ONLY` is inert inside it.
const PREREQ_IMAGE: &str = "harness-base:latest";

/// `/out` markers, the same two `cmds/run.rs` polls.
const PREREQ_READY: &str = "/out/prereqs.ready";
const PREREQ_FAILED: &str = "/out/prereqs.FAILED";
const PREREQ_LOG: &str = "/out/prereqs.log";

/// `experiment.toml`'s `runtime.prereq_timeout_s`.
const PREREQ_TIMEOUT: Duration = Duration::from_secs(180);

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

/// Removes the container on drop. A failing assertion unwinds past any trailing `docker rm -f`, and
/// after the image fix the leaked container is a *running* privileged one with an inner dockerd and
/// a postgres in it — so cleanup has to be tied to the scope, not to reaching the last line.
struct Cleanup(String);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", self.0.as_str()])
            .output();
    }
}

/// `docker inspect` the exit code and the first lines of `docker logs`, for a container that died.
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

/// Mirrors `cmds/run.rs`'s `wait_prereqs`: `FAILED` first so a broken prereq stage reports in about
/// a second instead of after the full timeout, then `ready`. Returns the reason on failure.
///
/// The liveness check is the one production does not need — `run` leaves a failed container up by
/// design — and is the one this test does: a container that exits writes neither marker, so without
/// it every "PID 1 was never `container-entrypoint`" defect costs the whole timeout and reports as a
/// missing file rather than as a dead entrypoint. That is the defect this test previously carried.
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

/// One privileged persistent container running the prereq stage — the entrypoint and prereq stage
/// `run` uses. Two deliberate deltas from a real dispatch: `container::launch_plan` also mounts
/// `/work /task:ro /progress /agent-home /out` and passes `--memory --cpus --pids-limit`, neither of
/// which the `PREREQ_ONLY` branch reads. No `/work` mount is supplied on purpose: `prereqs()` walks
/// `/work` for `**/tests/fixtures/seed.sql`, so bind-mounting the host's `/tmp` would make the test
/// depend on whatever is lying in the operator's temp directory. Missing seeds warn and continue.
#[test]
fn container_runs_prereq_stage_and_stays_up() {
    if !enabled() || !docker_available() {
        eprintln!("skipping: set TASKFMT_ITEST_DOCKER=1 (and build {PREREQ_IMAGE})");
        return;
    }
    if !docker::image_exists(PREREQ_IMAGE) {
        eprintln!("skipping: {PREREQ_IMAGE} not built");
        return;
    }

    // Per-process name. A fixed one plus the `docker rm -f` preamble below makes two concurrent
    // runs — a meta package's unfiltered `cargo test` and an operator's manual one — remove each
    // other's container mid-assertion, which reads like a runtime defect and is not one.
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
        .expect("docker run");
    let container = Cleanup(name);
    let name = container.0.as_str();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );

    // Positive evidence that the prereq stage ran and succeeded. `docker run -d` returns as soon as
    // the container is *started*, so asserting on liveness alone says nothing about what happened
    // inside it.
    if let Err(reason) = wait_prereqs(name) {
        panic!("{reason}");
    }

    // `PREREQ_ONLY=1` parks (`cmds/container_entrypoint.rs`) instead of launching the agent, so the
    // container is still up *after* a successful prereq stage.
    assert!(docker::is_running(name), "PREREQ_ONLY must park, not exit");

    // `/proc/1/cmdline` is NUL-separated. Both tokens are required: `harness-taskfmt:latest`'s PID 1
    // also "contains taskfmt", which is why the single-token form did not catch the wrong image.
    let entry = docker::read_file(name, "/proc/1/cmdline")
        .unwrap_or_default()
        .replace('\0', " ");
    assert!(
        entry.contains("taskfmt") && entry.contains("container-entrypoint"),
        "PID 1 must be `taskfmt container-entrypoint`: {entry:?}"
    );
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
