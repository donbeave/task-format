//! Exercise crash ownership with real OS processes; Docker itself is isolated by a stub.
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const FIXTURE_ENV: &str = "TASKFMT_SUPERVISOR_TEST_ROOT";

fn executable(path: &Path, source: &str) {
    std::fs::write(path, source).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn signal(target: &str, signal: &str) {
    assert!(
        Command::new("/bin/kill")
            .args([signal, "--", target])
            .status()
            .unwrap()
            .success()
    );
}

fn eventually(mut condition: impl FnMut() -> bool, message: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !condition() {
        assert!(Instant::now() < deadline, "{message}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

struct Owner(Child);
impl Drop for Owner {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct TaskCleanup(Option<u32>);
impl Drop for TaskCleanup {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", "--", &format!("-{pid}")])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

fn exercise_parent_signal(kill_parent: bool) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::create_dir(root.join("artifacts")).unwrap();
    std::fs::create_dir(root.join("bin")).unwrap();
    executable(&root.join("bin/docker"), "#!/bin/sh\nexit 0\n");
    // exec preserves the observed PID. The task never reaches container dispatch.
    executable(
        &root.join("taskfmt"),
        "#!/bin/sh\nprintf '%s' \"$$\" > \"$TASKFMT_SUPERVISOR_TEST_ROOT/child.pid\"\nexec /bin/sleep 120\n",
    );
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let paths = std::iter::once(root.join("bin")).chain(std::env::split_paths(&inherited));
    let path = std::env::join_paths(paths).unwrap();
    let mut owner = Owner(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "server::supervisor_tests::supervisor_parent_helper",
                "--nocapture",
            ])
            .env(FIXTURE_ENV, root)
            .env("PATH", path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .process_group(0)
            .spawn()
            .unwrap(),
    );
    eventually(
        || {
            std::fs::read_to_string(root.join("child.pid"))
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
                .is_some()
        },
        "supervised taskfmt never started",
    );
    let child_pid: u32 = std::fs::read_to_string(root.join("child.pid"))
        .unwrap()
        .parse()
        .unwrap();
    let mut child_cleanup = TaskCleanup(Some(child_pid));
    assert!(
        crate::monitor::RunLock::acquire(&root.join("artifacts")).is_err(),
        "supervisor must own lease before starting taskfmt"
    );
    if kill_parent {
        signal(&owner.0.id().to_string(), "-KILL");
    } else {
        // Mimic a terminal Ctrl-C delivered to the complete backend foreground group.
        signal(&format!("-{}", owner.0.id()), "-INT");
    }
    eventually(
        || owner.0.try_wait().unwrap().is_some(),
        "backend owner did not exit",
    );
    eventually(
        || {
            !Command::new("/bin/kill")
                .args(["-0", &child_pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success()
        },
        "taskfmt survived owner termination before container dispatch",
    );
    eventually(
        || crate::monitor::RunLock::acquire(&root.join("artifacts")).is_ok(),
        "supervisor did not release cleanup ownership",
    );
    child_cleanup.0 = None;
}

#[test]
fn backend_sigkill_stops_pre_dispatch_taskfmt_and_releases_lease() {
    exercise_parent_signal(true);
}

#[test]
fn foreground_sigint_preserves_supervisor_until_child_cleanup() {
    exercise_parent_signal(false);
}

#[test]
fn supervisor_parent_helper() {
    let Some(root) = std::env::var_os(FIXTURE_ENV).map(PathBuf::from) else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let (cancel, receiver) = tokio::sync::watch::channel(false);
        let config = super::adapter::AdapterConfig {
            taskfmt: root.join("taskfmt"),
            experiment_config: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../experiment.toml"),
            repo: "https://example.invalid/unused.git".into(), agent: None,
        };
        // Register before exposing the child.pid readiness signal to the outer test.
        let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).unwrap();
        let run = config.execute(root.join("unused-package"), root.join("artifacts"), receiver);
        tokio::pin!(run);
        tokio::select! {
            result = &mut run => panic!("task unexpectedly finished before parent signal: {result:?}"),
            _ = interrupt.recv() => {
                cancel.send_replace(true);
                assert!(run.await.is_err(), "cancelled task must not report verified completion");
            }
        }
    });
}
