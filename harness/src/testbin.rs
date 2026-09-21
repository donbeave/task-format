//! Locate workspace validation binaries for subprocess tests.
//!
//! Integration tests execute the real binaries (`taskfmt`, `taskfmt-host`)
//! to assert exit codes and CLI output. `cargo test -p` does not build other
//! packages' binaries, so resolve the binary anchored at this test
//! executable's own target directory and build it on demand when absent.
//! (Nightly `artifact = "bin"` dev-dependencies would obsolete this helper;
//! the workspace is stable-only.)

use std::path::PathBuf;
use std::process::Command;

/// Path to package `package`'s built `bin` binary, building it first when
/// absent.
///
/// Prefers `CARGO_BIN_EXE_<bin>` when cargo exports it, else anchors at the
/// test executable's sibling directory (robust to `CARGO_TARGET_DIR` and
/// cache-view target overrides, unlike manifest-relative `target/debug`).
pub fn built_bin(package: &str, bin: &str) -> PathBuf {
    for key in [
        format!("CARGO_BIN_EXE_{bin}"),
        format!("CARGO_BIN_EXE_{}", bin.replace('-', "_")),
    ] {
        if let Ok(path) = std::env::var(&key) {
            return PathBuf::from(path);
        }
    }
    let file = format!("{bin}{}", std::env::consts::EXE_SUFFIX);
    let anchored = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent()?.parent().map(|dir| dir.join(&file)));
    let path = anchored.unwrap_or_else(|| PathBuf::from("target").join("debug").join(&file));
    if !path.exists() {
        let output = Command::new(env!("CARGO"))
            .args([
                "build",
                "--locked",
                "--offline",
                "-p",
                package,
                "--bin",
                bin,
            ])
            .output()
            .unwrap_or_else(|err| {
                panic!("cargo build -p {package} --bin {bin} failed to spawn: {err}")
            });
        assert!(
            output.status.success() && path.exists(),
            "cargo build -p {package} --bin {bin} failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    path
}
