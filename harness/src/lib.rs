//! `taskfmt` — the Rust replacement for every shell tool under `harness/`.
//!
//! Library modules hold the logic (lint, progress, gate, dispatch); `cli` + `main` only parse
//! arguments and dispatch. Integration tests in `tests/` drive the library directly.

pub mod acceptance;
pub mod cli;
pub mod cmds;
pub mod config;
pub mod fingerprint;
pub mod gate;
pub mod interactive;
pub mod itest;
pub mod lint;
pub mod monitor;
pub mod ops;
pub mod progress;
pub mod redact;
pub mod runstate;
pub mod selection;
pub mod selfcheck;
pub mod selfhost;
pub mod selftest;
pub mod server;
pub mod study;
pub mod taskfile;
pub mod verifycfg;

/// The full Git commit that produced this binary, or `unknown` for a source tree without Git.
pub const GIT_COMMIT_SHA: &str = env!("TASKFMT_GIT_COMMIT_SHA");

/// Human-readable binary identity shown by `taskfmt --version` and recorded in promotions.
pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (git ",
    env!("TASKFMT_GIT_COMMIT_SHA"),
    ")"
);

/// The content fingerprint of the hash input set this binary was compiled from — `Cargo.toml`,
/// `Cargo.lock`, `build.rs` and every file under `src/` — as 64 lowercase hex digits, baked in by
/// `build.rs`.
///
/// It is the comparand `taskfmt run` checks against the gate baked into the agent image. The
/// `VERSION` string separately identifies the producing Git commit, while this digest remains the
/// source-based host/image compatibility check.
pub const HARNESS_FINGERPRINT: &str = env!("TASKFMT_HARNESS_FINGERPRINT");
