//! Shared taskfmt library: lint, progress, gate, and parsers.

pub mod acceptance;
pub mod config;
pub mod executioncfg;
pub mod gate;
pub mod hash;
pub mod lint;
pub mod ops;
pub mod progress;
pub mod redact;
pub mod selfcheck;
pub mod selftest;
pub mod taskfile;
pub mod verifycfg;

/// The full Git commit that produced this binary, or `unknown` for a source tree without Git.
pub const GIT_COMMIT_SHA: &str = env!("TASKFMT_GIT_COMMIT_SHA");

/// Human-readable binary identity shown by `--version` and recorded in promotions.
pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (git ",
    env!("TASKFMT_GIT_COMMIT_SHA"),
    ")"
);
