//! Trusted TASK-007 gate: exit code 0 and terminal restore under a real pty (D-040).

mod support;

use nix::pty::{Winsize, openpty};
use nix::sys::termios::{SetArg, cfmakeraw, tcgetattr, tcsetattr};
use std::fs::File;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// crossterm leaves the alternate screen with this sequence.
const LEAVE_ALTERNATE_SCREEN: &str = "\x1b[?1049l";

/// Owns the spawned child for the whole of `run_in_pty`. Every exit path — the deadline below, a
/// panic from any `expect` after the spawn, an ordinary return — terminates and reaps it, so a
/// failing case costs its own deadline and never leaves a live process holding the pty slave.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Runs the binary on a 100x30 pty, sends `keys`, returns (exit code, output).
fn run_in_pty(db: &std::path::Path, keys: &[u8]) -> (i32, String) {
    let pty = openpty(
        Some(&Winsize {
            ws_row: 30,
            ws_col: 100,
            ws_xpixel: 0,
            ws_ypixel: 0,
        }),
        None,
    )
    .expect("trusted support: openpty");

    // The keys below are written without waiting for the child, and no conforming implementation can
    // enable raw mode before `spawn` returns (D-012 fixes the startup order). Under the default line
    // discipline the driver consumes `\x03` as the interrupt character and it is never delivered as
    // data, so put the terminal in raw mode here, while no child exists to race with.
    let mut termios = tcgetattr(&pty.slave).expect("trusted support: tcgetattr");
    cfmakeraw(&mut termios);
    tcsetattr(&pty.slave, SetArg::TCSANOW, &termios).expect("trusted support: tcsetattr");

    let writer = File::from(pty.master.try_clone().expect("dup master"));
    let reader = File::from(pty.master);
    let stdin = File::from(pty.slave.try_clone().expect("dup slave"));
    let stdout = File::from(pty.slave);

    let mut child = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_pgtui"))
            .arg("--db")
            .arg(db)
            .env_remove("PGTUI_DB")
            .stdin(Stdio::from(stdin))
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn pgtui"),
    );

    let (out_tx, out_rx) = mpsc::channel::<Vec<u8>>();
    let reader_thread = std::thread::spawn(move || {
        let mut reader = reader;
        let mut chunk = [0_u8; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if out_tx.send(chunk[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let mut writer = writer;
    writer.write_all(keys).expect("write keys");
    let _ = writer.flush();

    let deadline = Instant::now() + Duration::from_secs(30);
    let code = loop {
        if let Some(status) = child.0.try_wait().expect("wait for pgtui") {
            match status.code() {
                Some(code) => break code,
                None => panic!("pgtui was killed by a signal"),
            }
        }
        if Instant::now() >= deadline {
            // Terminate before reporting: the child holds the only remaining pty slave fd, so
            // reaping it is what lets the reader below reach end-of-input.
            let _ = child.0.kill();
            let _ = child.0.wait();
            panic!("pgtui did not exit within 30 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    let mut output = Vec::new();
    let drain_until = Instant::now() + Duration::from_secs(2);
    while Instant::now() < drain_until {
        match out_rx.try_recv() {
            Ok(chunk) => {
                output.extend_from_slice(&chunk);
            }
            Err(mpsc::TryRecvError::Empty) => std::thread::sleep(Duration::from_millis(25)),
            Err(mpsc::TryRecvError::Disconnected) => break,
        }
    }
    let _ = reader_thread.join();
    (code, String::from_utf8_lossy(&output).to_string())
}

/// A fresh store path in a directory that lives as long as the test.
fn fresh_db() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("trusted support: tempdir");
    let path = dir.path().join("connections.db");
    (dir, path)
}

#[test]
fn q_exits_with_zero() {
    let (_dir, db) = fresh_db();
    let (code, output) = run_in_pty(&db, b"q");
    assert_eq!(code, 0, "exit code, output: {output:?}");
    assert!(
        output.contains(LEAVE_ALTERNATE_SCREEN),
        "terminal restored: {:?}",
        output
    );
}

#[test]
fn ctrl_c_exits_with_zero() {
    let (_dir, db) = fresh_db();
    let (code, output) = run_in_pty(&db, b"\x03");
    assert_eq!(code, 0, "exit code, output: {output:?}");
    assert!(
        output.contains(LEAVE_ALTERNATE_SCREEN),
        "terminal restored: {:?}",
        output
    );
}
