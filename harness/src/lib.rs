//! `taskfmt` — the Rust replacement for every shell tool under `harness/`.
//!
//! Library modules hold the logic (lint, progress, gate, dispatch); `cli` + `main` only parse
//! arguments and dispatch. Integration tests in `tests/` drive the library directly.

pub mod cli;
pub mod cmds;
pub mod config;
pub mod gate;
pub mod interactive;
pub mod lint;
pub mod ops;
pub mod progress;
pub mod redact;
pub mod runstate;
pub mod selection;
pub mod selftest;
pub mod taskfile;
pub mod verifycfg;
