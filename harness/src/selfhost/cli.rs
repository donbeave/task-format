//! `SelfhostCmd` — the clap `Subcommand` enum and EVERY flag the driver takes (plan §8.1).
//!
//! `harness/src/cli.rs` is fenced and carries one delegating variant with no flag on it, so this
//! enum can grow without reopening it. Two subcommands ship here and no more (TASK-114 R-013):
//! `step`, `probe`, `gc`, `env-fault`, `token`, `reset`, `supervise`, `freeze`, `diff-check` and
//! `verdict --final` are a successor's.

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

#[derive(Subcommand, Debug)]
pub enum SelfhostCmd {
    /// Fold the ledger and print the bounded state table.
    Status(StatusArgs),
    /// Append one typed record to the ledger.
    Record(RecordArgs),
}

/// The corpus a verdict file belongs to. Decided by `--corpus` and cross-checked against where the
/// file lives, never inferred from the task id (plan §4.6.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Corpus {
    Experiment,
    Meta,
    Calibration,
}

impl Corpus {
    pub fn as_str(self) -> &'static str {
        match self {
            Corpus::Experiment => "experiment",
            Corpus::Meta => "meta",
            Corpus::Calibration => "calibration",
        }
    }
}

/// The flags every subcommand that reads a ledger takes.
#[derive(Args, Debug)]
pub struct CommonArgs {
    /// This repository's root — the directory holding `experiment.toml`, `selfhost/state/` and
    /// `selfhost/reports/`. Precedes the manifest discovery order, and does not replace it.
    #[arg(long)]
    pub meta_root: Option<PathBuf>,

    /// The subject checkout. Precedes $TASKFORMAT_REPO and the sibling/nested candidates.
    #[arg(long)]
    pub subject_root: Option<PathBuf>,

    /// Which ledger under `selfhost/state/`. Default: the single non-reserved candidate, else the
    /// lexicographically greatest.
    #[arg(long)]
    pub repo_id: Option<String>,

    /// Select the Phase 1 relaxed reader. WITHOUT THIS FLAG EVERY READ IS STRICT (plan §4.6.7).
    #[arg(long)]
    pub phase1: bool,
}

#[derive(Args, Debug)]
pub struct StatusArgs {
    #[command(flatten)]
    pub common: CommonArgs,

    /// Print exactly one line: one of plan §4.6.7's six strings. The only writer of the
    /// `terminal` record, at most once per (cycle, reason).
    #[arg(long)]
    pub sentinel: bool,
}

#[derive(Args, Debug)]
pub struct RecordArgs {
    #[command(flatten)]
    pub common: CommonArgs,

    /// A `selfhost.verdict/v1` file, already written by the verifier where it lives. `overall` and
    /// the digest are read from the file, never from a flag.
    #[arg(
        long,
        requires = "corpus",
        conflicts_with = "note",
        required_unless_present = "note"
    )]
    pub verdict_file: Option<PathBuf>,

    /// Which corpus the verdict belongs to.
    #[arg(long, value_enum, requires = "verdict_file")]
    pub corpus: Option<Corpus>,

    /// Required on the calibration corpus; must equal the file's basename without `.json`.
    #[arg(long, requires = "verdict_file")]
    pub fixture: Option<String>,

    /// Write to the Phase 1 ledger, `selfhost/state/preflight/ledger.jsonl`, creating it at seq 1.
    /// Arm-independent.
    #[arg(long)]
    pub preflight: bool,

    /// An operator note: the only route by which a pre-schema Phase 1 round enters the ledger.
    #[arg(long)]
    pub note: Option<String>,

    /// A file the note is about. Repeatable; each is recorded by path and SHA-256.
    #[arg(long, requires = "note")]
    pub artifact: Vec<PathBuf>,
}
