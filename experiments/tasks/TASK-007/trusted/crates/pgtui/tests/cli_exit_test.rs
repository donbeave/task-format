//! Trusted TASK-007 gate: exit code 0 and terminal restore under a real pty (D-040).

mod support;

use nix::pty::{Winsize, openpty};
use std::fs::File;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// crossterm leaves the alternate screen with this sequence.
const LEAVE_ALTERNATE_SCREEN: &str = "\x1b[?1049l";

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

    let writer = File::from(pty.master.try_clone().expect("dup master"));
    let reader = File::from(pty.master);
    let stdin = File::from(pty.slave.try_clone().expect("dup slave"));
    let stdout = File::from(pty.slave);

    let mut child = Command::new(env!("CARGO_BIN_EXE_pgtui"))
        .arg("--db")
        .arg(db)
        .env_remove("PGTUI_DB")
        .stdin(Stdio::from(stdin))
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn pgtui");

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

    let (code_tx, code_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let code = child.wait().ok().and_then(|status| status.code());
        let _ = code_tx.send(code);
    });

    let deadline = Instant::now() + Duration::from_secs(30);
    let code = loop {
        match code_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Some(code)) => break code,
            Ok(None) => panic!("pgtui was killed by a signal"),
            Err(_) => assert!(Instant::now() < deadline, "pgtui did not exit within 30 s"),
        }
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
