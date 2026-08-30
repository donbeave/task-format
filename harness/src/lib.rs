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
pub mod lint;
pub mod ops;
pub mod progress;
pub mod redact;
pub mod runstate;
pub mod selection;
pub mod selfcheck;
pub mod selfhost;
pub mod selftest;
pub mod taskfile;
pub mod verifycfg;

/// The content fingerprint of the hash input set this binary was compiled from — `Cargo.toml`,
/// `Cargo.lock`, `build.rs` and every file under `src/` — as 64 lowercase hex digits, baked in by
/// `build.rs`.
///
/// It is the comparand `taskfmt run` checks against the gate baked into the agent image: the
/// crate `version` is exactly what a forgotten reinstall leaves untouched, and the two binaries'
/// own sha256s are a Mach-O and an ELF and never match. This one is computed from text, so it is
/// the same on both sides whenever the sources are.
pub const HARNESS_FINGERPRINT: &str = env!("TASKFMT_HARNESS_FINGERPRINT");
