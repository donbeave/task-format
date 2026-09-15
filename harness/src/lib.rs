//! `taskfmt` — the Rust replacement for every shell tool under `harness/`.
//!
//! Library modules hold the logic (lint, progress, gate, dispatch); `cli` + `main` only parse
//! arguments and dispatch. Integration tests in `tests/` drive the library directly.

pub mod acceptance;
pub mod cli;
pub mod cmds;
pub mod config;
pub mod executioncfg;
pub mod gate;
pub mod hash;
pub mod interactive;
pub mod itest;
pub mod lint;
pub mod ops;
pub mod progress;
pub mod redact;
pub mod runstate;
pub mod selection;
pub mod selfcheck;
pub mod selftest;
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
