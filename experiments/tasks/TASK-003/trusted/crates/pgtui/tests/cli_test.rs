//! Trusted TASK-002 gate: CLI, store path handling and exit codes (D-020, D-040).

mod support;

use std::process::Command;

#[test]
fn unwritable_db_exits_2() {
    let guard = support::unwritable_db_path();
    let db = guard.path().join("missing-dir").join("connections.db");
    let output = Command::new(env!("CARGO_BIN_EXE_pgtui"))
        .arg("--db")
        .arg(&db)
        .env_remove("PGTUI_DB")
        .output()
        .expect("pgtui runs");
    assert_eq!(output.status.code(), Some(2), "status: {:?}", output.status);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error:"), "stderr: {stderr}");
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("error:"),
        "errors go to stderr"
    );
}

#[test]
fn version_exits_0() {
    let output = Command::new(env!("CARGO_BIN_EXE_pgtui"))
        .arg("--version")
        .env_remove("PGTUI_DB")
        .output()
        .expect("pgtui runs");
    assert_eq!(output.status.code(), Some(0), "status: {:?}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("pgtui"), "stdout: {stdout}");
}

#[test]
fn help_exits_0() {
    let output = Command::new(env!("CARGO_BIN_EXE_pgtui"))
        .arg("--help")
        .env_remove("PGTUI_DB")
        .output()
        .expect("pgtui runs");
    assert_eq!(output.status.code(), Some(0), "status: {:?}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--db"), "--db documented: {stdout}");
}
