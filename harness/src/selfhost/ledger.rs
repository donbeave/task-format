//! `selfhost.ledger/v1` — the record types, the two readers, and the ONE writer (plan §4.6.2).
//!
//! One file, `<meta_root>/selfhost/state/<repo-id>/ledger.jsonl`, append-only, one compact JSON
//! object per line, LF terminated. [`append`] is the only function in this tree that opens it for
//! writing, which is the mechanical half of plan §4.4's invariant 5.
//!
//! **Two readers and nothing between them** (plan §4.6.7, TASK-114 D-006). [`Reader::Strict`] is
//! §4.6.2b's ledger half: an unknown top-level key, an unknown `kind` or an unknown `schema` is a
//! refusal, because the only writer is [`append`] and so an unknown key has exactly one cause.
//! [`Reader::Phase1Lenient`] is selected by `--phase1` and by nothing else; it accepts the
//! hand-kept §3.2.1 ledgers, preserves what it does not know, and announces every DEVIATING
//! record once on stderr.

use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::Path;

use anyhow::Context;
use serde_json::{Map, Value};

use crate::redact;

/// The only accepted `schema` value.
pub const SCHEMA: &str = "selfhost.ledger/v1";

/// The five-field envelope every record carries (plan §4.6.2).
pub const ENVELOPE: [&str; 5] = ["schema", "kind", "ts", "seq", "cycle"];

/// The closed enum of fifteen kinds. A sixteenth is a plan change, not an implementation choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    CycleOpen,
    DispatchIntent,
    Attempt,
    Verdict,
    Heartbeat,
    EnvFault,
    Diagnosis,
    Debate,
    Token,
    Reset,
    Revoke,
    Gc,
    Freeze,
    Note,
    Terminal,
}

/// Every kind, in the order plan §4.6.2's table lists them.
pub const KINDS: [Kind; 15] = [
    Kind::CycleOpen,
    Kind::DispatchIntent,
    Kind::Attempt,
    Kind::Verdict,
    Kind::Heartbeat,
    Kind::EnvFault,
    Kind::Diagnosis,
    Kind::Debate,
    Kind::Token,
    Kind::Reset,
    Kind::Revoke,
    Kind::Gc,
    Kind::Freeze,
    Kind::Note,
    Kind::Terminal,
];

impl Kind {
    /// The wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::CycleOpen => "cycle_open",
            Kind::DispatchIntent => "dispatch_intent",
            Kind::Attempt => "attempt",
            Kind::Verdict => "verdict",
            Kind::Heartbeat => "heartbeat",
            Kind::EnvFault => "env_fault",
            Kind::Diagnosis => "diagnosis",
            Kind::Debate => "debate",
            Kind::Token => "token",
            Kind::Reset => "reset",
            Kind::Revoke => "revoke",
            Kind::Gc => "gc",
            Kind::Freeze => "freeze",
            Kind::Note => "note",
            Kind::Terminal => "terminal",
        }
    }

    /// The wire spelling back to a kind. `None` is an unknown kind, which both readers refuse.
    pub fn parse(text: &str) -> Option<Kind> {
        KINDS.into_iter().find(|kind| kind.as_str() == text)
    }

    /// The kind-specific top-level keys, beyond the envelope, exactly as plan §4.6.2 fixes them.
    ///
    /// `verdict`'s set is the UNION of the three corpus forms, with `diagnosis_id` carried `null`
    /// off the meta corpus; `note` and `freeze` are the two shapes §4.6.19 item 26 added, without
    /// which "unknown top-level key" had no predicate for them at all.
    pub fn fields(self) -> &'static [&'static str] {
        match self {
            Kind::CycleOpen => &[
                "repo_url",
                "repo_id",
                "experiment_id",
                "bootstrap_sha",
                "reason",
                "green_field_cause",
                "predecessor_cycle",
                "discarded_verified",
                "harness_sha",
                "selfhost_sha",
                "window_start",
                "window_end",
            ],
            Kind::DispatchIntent => &[
                "task",
                "contract",
                "tuple",
                "image_ids",
                "taskfmt_path",
                "taskfmt_fingerprint_host",
                "taskfmt_fingerprint_image",
                "dispatch_counter",
                "repairs_landed",
                "kill_after_min",
                "agent",
                "base_sha",
            ],
            Kind::Attempt => &[
                "task",
                "run_id",
                "run_dir",
                "container",
                "base_sha",
                "result_sha",
                "gate",
                "status_state",
                "started",
                "finished",
                "verdict_file",
                "verdict_sha256",
                "agent",
                "model",
                "effort",
                "operator_note",
                "tuple",
                "intent_seq",
            ],
            Kind::Verdict => &[
                "task",
                "corpus",
                "fixture",
                "attempt_seq",
                "verdict_file",
                "verdict_sha256",
                "overall",
                "verifier",
                "pushed_sha",
                "base_sha",
                "gh_check",
                "counts",
                "extra_keys",
                "decisive_evidence",
                "diagnosis_id",
            ],
            Kind::Heartbeat => &["run_id", "status_state"],
            Kind::EnvFault => &[
                "reason",
                "class",
                "terminal",
                "http_status",
                "provider",
                "raw",
                "source",
                "backoff_index",
                "next_retry_after_s",
            ],
            Kind::Diagnosis => &[
                "diagnosis_id",
                "task",
                "attempt_seq",
                "supersedes",
                "framing",
                "fault_class",
                "earliest_bad_task",
                "file",
                "file_sha256",
                "proposed_paths",
                "replay_scope",
                "round",
                "package_repo",
                "package_id",
                "package_commit",
                "package_tree_hash",
                "package_worktree_clean",
                "extra_keys",
            ],
            Kind::Debate => &[
                "diagnosis_id",
                "round",
                "reviewer",
                "framing",
                "position",
                "artifact",
                "artifact_sha256",
                "extra_keys",
                "falsifier",
                "falsifier_reproduced",
                "dissent_text",
            ],
            Kind::Token => &[
                "diagnosis_id",
                "repo",
                "path",
                "reviewer_verdict",
                "reviewer_verdict_sha256",
                "token",
                "debate_round_seqs",
            ],
            Kind::Reset => &[
                "to_task",
                "to_sha",
                "expected_remote_sha",
                "orphaned",
                "dropped_verified",
                "experiment_json",
                "dropped_task_entries",
                "remote",
            ],
            Kind::Revoke => &["earliest_bad_task", "dropped", "diagnosis_id", "reason"],
            Kind::Gc => &["keep_last", "reaped", "protected", "bytes_freed"],
            Kind::Freeze => &[
                "freeze_file",
                "freeze_sha256",
                "tag",
                "harness_sha",
                "selfhost_sha",
                "package_tree_hash",
                "image_digests",
                "image_ids",
                "taskfmt_binary_sha256",
                "taskfmt_path",
                "taskfmt_fingerprint_host",
                "taskfmt_fingerprint_image",
                "agents",
                "model",
                "effort",
            ],
            Kind::Note => &["text", "artifacts"],
            Kind::Terminal => &["sentinel", "reason", "failed_at", "basis"],
        }
    }

    /// Envelope plus [`Kind::fields`] — the set "unknown top-level key" is decided against.
    pub fn canonical_keys(self) -> BTreeSet<&'static str> {
        ENVELOPE
            .into_iter()
            .chain(self.fields().iter().copied())
            .collect()
    }
}

/// Which reader a caller selected. `--phase1` picks the second and nothing else does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reader {
    Strict,
    Phase1Lenient,
}

impl Reader {
    /// The spelling recorded in a `terminal` record's `basis.reader` (plan §4.6.7).
    pub fn as_str(self) -> &'static str {
        match self {
            Reader::Strict => "strict",
            Reader::Phase1Lenient => "phase1-lenient",
        }
    }

    /// `--phase1` announces itself on stderr, never on stdout: `--sentinel` owns stdout's one line.
    pub fn announce(self) {
        if self == Reader::Phase1Lenient {
            redact::eemit(
                "READER phase1-lenient (plan §4.6.7; unknown keys preserved, not refused)",
            );
        }
    }
}

/// How one record's key set differs from its kind's canonical set, in both directions.
#[derive(Clone, Debug, Default)]
pub struct Deviation {
    pub unknown: Vec<String>,
    pub missing: Vec<String>,
}

impl Deviation {
    pub fn is_empty(&self) -> bool {
        self.unknown.is_empty() && self.missing.is_empty()
    }
}

/// One parsed line: the typed envelope plus the object exactly as it was written.
///
/// The whole object is kept because the relaxed reader must PRESERVE what it does not know, and
/// because the deviation test is over the raw key set.
#[derive(Clone, Debug)]
pub struct Record {
    pub kind: Kind,
    pub seq: u64,
    pub cycle: Option<String>,
    pub ts: String,
    pub line: usize,
    pub obj: Map<String, Value>,
}

impl Record {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.obj.get(key).filter(|value| !value.is_null())
    }

    pub fn str_field(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn u64_field(&self, key: &str) -> Option<u64> {
        self.get(key).and_then(Value::as_u64)
    }

    pub fn bool_field(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Value::as_bool)
    }

    /// The keys this record carries beyond its kind's set, and the ones it is missing.
    ///
    /// A record DEVIATES iff either list is non-empty (TASK-114 D-005). The strict reader's
    /// refusal predicate is deliberately NARROWER — unknown keys only — because the two answer
    /// two questions: "did something other than `append` write this" and "how does this differ
    /// from the schema".
    pub fn deviation(&self) -> Deviation {
        let canonical = self.kind.canonical_keys();
        let present: BTreeSet<&str> = self.obj.keys().map(String::as_str).collect();
        Deviation {
            unknown: present
                .difference(&canonical)
                .map(|key| (*key).to_string())
                .collect(),
            missing: canonical
                .difference(&present)
                .map(|key| (*key).to_string())
                .collect(),
        }
    }
}

/// A read that refused, carrying the exact stderr line the subcommand prints (TASK-114 D-007).
#[derive(Debug)]
pub struct Refusal(pub String);

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a read can fail with: the ledger's own refusal, or an I/O or parse fault.
#[derive(Debug)]
pub enum ReadError {
    Refused(Refusal),
    Fault(anyhow::Error),
}

impl From<anyhow::Error> for ReadError {
    fn from(err: anyhow::Error) -> Self {
        ReadError::Fault(err)
    }
}

fn refuse(reason: &str) -> ReadError {
    ReadError::Refused(Refusal(format!("LEDGER-REFUSED {reason}")))
}

/// Read every complete line of a ledger under the selected reader.
///
/// A missing file reads as an empty ledger — `record --preflight` creates one at `seq` 1. A torn
/// final line (one with no terminating LF) is skipped without an error, because a partial write is
/// a crash artifact; a malformed COMPLETE line is a fault, because a record that does not parse is
/// a record whose meaning nobody knows.
pub fn read_all(path: &Path, reader: Reader) -> Result<Vec<Record>, ReadError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read {}", path.display()))
        .map_err(ReadError::Fault)?;
    let mut complete: Vec<&str> = text.split('\n').collect();
    // `split` always yields a final element; it is "" when the file ends with LF and the torn
    // remainder when it does not. Either way it is not a complete line.
    complete.pop();

    let mut records = Vec::new();
    for (index, raw) in complete.into_iter().enumerate() {
        let line = index + 1;
        if raw.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(raw)
            .with_context(|| format!("malformed complete line {line} in {}", path.display()))
            .map_err(ReadError::Fault)?;
        let Value::Object(obj) = value else {
            return Err(ReadError::Fault(anyhow::anyhow!(
                "line {line} in {} is not a JSON object",
                path.display()
            )));
        };
        let schema = obj.get("schema").and_then(Value::as_str).unwrap_or("");
        if schema != SCHEMA {
            return Err(refuse(&format!(
                "unknown-schema {schema:?} line={line} file={}",
                path.display()
            )));
        }
        let kind_text = obj.get("kind").and_then(Value::as_str).unwrap_or("");
        let Some(kind) = Kind::parse(kind_text) else {
            return Err(refuse(&format!(
                "unknown-kind {kind_text:?} line={line} file={}",
                path.display()
            )));
        };
        let record = Record {
            kind,
            seq: obj
                .get("seq")
                .and_then(Value::as_u64)
                .unwrap_or(line as u64),
            cycle: obj.get("cycle").and_then(Value::as_str).map(str::to_string),
            ts: obj
                .get("ts")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            line,
            obj,
        };
        let deviation = record.deviation();
        match reader {
            Reader::Strict => {
                if let Some(key) = deviation.unknown.first() {
                    return Err(refuse(&format!(
                        "unknown-key {key} kind={} seq={} line={line} file={}",
                        record.kind.as_str(),
                        record.seq,
                        path.display()
                    )));
                }
            }
            Reader::Phase1Lenient => {
                if !deviation.is_empty() {
                    redact::eemit(&format!(
                        "SCHEMA-LENIENT seq={} kind={} line={line} unknown=[{}] missing=[{}]",
                        record.seq,
                        record.kind.as_str(),
                        deviation.unknown.join(","),
                        deviation.missing.join(",")
                    ));
                }
            }
        }
        records.push(record);
    }
    Ok(records)
}

/// One more than the highest `seq` in the file; 1 for a ledger that does not exist yet.
pub fn next_seq(records: &[Record]) -> u64 {
    records.iter().map(|record| record.seq).max().unwrap_or(0) + 1
}

/// **The only function in this tree that opens a ledger for writing** (plan §4.4 invariant 5).
///
/// The record's key set must equal its kind's canonical set exactly — a kind whose writer
/// sometimes omits a field has no fixed key set, and the strict reader would then have nothing to
/// decide against. The line is scrubbed, written whole in one `write_all`, and `sync_data`d.
pub fn append(
    path: &Path,
    kind: Kind,
    seq: u64,
    cycle: Option<&str>,
    fields: Map<String, Value>,
) -> anyhow::Result<()> {
    let mut obj = Map::new();
    obj.insert("schema".into(), Value::String(SCHEMA.into()));
    obj.insert("kind".into(), Value::String(kind.as_str().into()));
    obj.insert(
        "ts".into(),
        Value::String(crate::config::timestamp_rfc3339()),
    );
    obj.insert("seq".into(), Value::from(seq));
    obj.insert(
        "cycle".into(),
        match cycle {
            Some(id) => Value::String(id.to_string()),
            None => Value::Null,
        },
    );
    for (key, value) in fields {
        if ENVELOPE.contains(&key.as_str()) {
            anyhow::bail!("refusing to write {key:?}: the envelope is this function's to fill");
        }
        obj.insert(key, value);
    }
    let canonical = kind.canonical_keys();
    let present: BTreeSet<&str> = obj.keys().map(String::as_str).collect();
    let unknown: Vec<&str> = present.difference(&canonical).copied().collect();
    let missing: Vec<&str> = canonical.difference(&present).copied().collect();
    anyhow::ensure!(
        unknown.is_empty() && missing.is_empty(),
        "refusing to write a {} record whose key set is not its kind's: unknown={unknown:?} \
         missing={missing:?}",
        kind.as_str()
    );

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    let line = redact::scrub(&serde_json::to_string(&Value::Object(obj))?);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("cannot open {} to append", path.display()))?;
    file.write_all(format!("{line}\n").as_bytes())
        .with_context(|| format!("cannot append to {}", path.display()))?;
    file.sync_data()
        .with_context(|| format!("cannot flush {}", path.display()))?;
    Ok(())
}

/// A complete field map for `kind`, every value `null` — the base every writer fills in, so no
/// record can be written with a key missing.
pub fn null_fields(kind: Kind) -> Map<String, Value> {
    kind.fields()
        .iter()
        .map(|key| ((*key).to_string(), Value::Null))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, lines: &[&str]) -> std::path::PathBuf {
        let path = dir.join("ledger.jsonl");
        std::fs::write(&path, lines.join("\n") + "\n").unwrap();
        path
    }

    /// One schema-complete record of every kind, built from the canonical set itself.
    fn complete_line(kind: Kind, seq: u64) -> String {
        let mut fields = null_fields(kind);
        if kind == Kind::Note {
            fields.insert("text".into(), Value::String("t".into()));
            fields.insert("artifacts".into(), Value::Array(vec![]));
        }
        let mut obj = Map::new();
        obj.insert("schema".into(), Value::String(SCHEMA.into()));
        obj.insert("kind".into(), Value::String(kind.as_str().into()));
        obj.insert("ts".into(), Value::String("2026-01-01T00:00:00Z".into()));
        obj.insert("seq".into(), Value::from(seq));
        obj.insert("cycle".into(), Value::String("exp-t".into()));
        for (key, value) in fields {
            obj.insert(key, value);
        }
        serde_json::to_string(&Value::Object(obj)).unwrap()
    }

    #[test]
    fn the_enum_is_closed_at_fifteen_and_every_kind_round_trips() {
        assert_eq!(KINDS.len(), 15);
        let names: BTreeSet<&str> = KINDS.iter().map(|kind| kind.as_str()).collect();
        assert_eq!(names.len(), 15, "the wire spellings are distinct");
        for kind in KINDS {
            assert_eq!(Kind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(Kind::parse("cycle_close"), None);
    }

    #[test]
    fn every_kind_reads_back_under_the_strict_reader() {
        let dir = tempfile::tempdir().unwrap();
        let lines: Vec<String> = KINDS
            .iter()
            .enumerate()
            .map(|(i, kind)| complete_line(*kind, i as u64 + 1))
            .collect();
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let path = write(dir.path(), &refs);
        let records = read_all(&path, Reader::Strict).expect("schema-complete lines are accepted");
        assert_eq!(records.len(), 15);
        for (record, kind) in records.iter().zip(KINDS) {
            assert_eq!(record.kind, kind);
            assert!(record.deviation().is_empty(), "{} deviates", kind.as_str());
        }
    }

    #[test]
    fn the_canonical_sets_are_the_sizes_the_plan_states() {
        assert_eq!(Kind::Note.canonical_keys().len(), 7);
        assert_eq!(Kind::Terminal.canonical_keys().len(), 9);
        assert_eq!(Kind::Verdict.canonical_keys().len(), 20);
        assert_eq!(Kind::Freeze.canonical_keys().len(), 20);
        assert_eq!(Kind::CycleOpen.canonical_keys().len(), 17);
        assert_eq!(Kind::Attempt.canonical_keys().len(), 23);
        assert_eq!(Kind::Diagnosis.canonical_keys().len(), 23);
        // `diagnosis_id` is a MEMBER of `verdict`'s set: the union of the three corpus forms
        assert!(Kind::Verdict.canonical_keys().contains("diagnosis_id"));
    }

    #[test]
    fn strict_refuses_an_unknown_top_level_key_naming_it() {
        let dir = tempfile::tempdir().unwrap();
        let line = r#"{"schema":"selfhost.ledger/v1","kind":"note","ts":"t","seq":1,"cycle":null,"text":"x","artifacts":[],"bogus_key":1}"#;
        let path = write(dir.path(), &[line]);
        match read_all(&path, Reader::Strict) {
            Err(ReadError::Refused(Refusal(msg))) => {
                assert!(
                    msg.starts_with("LEDGER-REFUSED unknown-key bogus_key"),
                    "{msg}"
                );
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        // the same line is ACCEPTED by the relaxed reader, which is the whole of the difference
        assert_eq!(read_all(&path, Reader::Phase1Lenient).unwrap().len(), 1);
    }

    #[test]
    fn both_readers_refuse_an_unknown_kind_and_an_unknown_schema() {
        let dir = tempfile::tempdir().unwrap();
        let bad_kind =
            r#"{"schema":"selfhost.ledger/v1","kind":"cycle_close","ts":"t","seq":1,"cycle":null}"#;
        let path = write(dir.path(), &[bad_kind]);
        for reader in [Reader::Strict, Reader::Phase1Lenient] {
            match read_all(&path, reader) {
                Err(ReadError::Refused(Refusal(msg))) => {
                    assert!(msg.contains("unknown-kind \"cycle_close\""), "{msg}")
                }
                other => panic!("expected a refusal, got {other:?}"),
            }
        }
        let bad_schema = r#"{"schema":"selfhost.ledger/v2","kind":"note","ts":"t","seq":1,"cycle":null,"text":"x","artifacts":[]}"#;
        let path = write(dir.path(), &[bad_schema]);
        match read_all(&path, Reader::Strict) {
            Err(ReadError::Refused(Refusal(msg))) => {
                assert!(
                    msg.contains("unknown-schema \"selfhost.ledger/v2\""),
                    "{msg}"
                )
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_field_is_not_a_strict_refusal_but_is_a_deviation() {
        let dir = tempfile::tempdir().unwrap();
        // the hand-kept shape of the committed seq-4 note: no `artifacts` at all
        let line = r#"{"schema":"selfhost.ledger/v1","kind":"note","ts":"t","seq":4,"cycle":"c","text":"x"}"#;
        let path = write(dir.path(), &[line]);
        let records = read_all(&path, Reader::Strict).expect("a missing field is not a refusal");
        let deviation = records[0].deviation();
        assert!(deviation.unknown.is_empty());
        assert_eq!(deviation.missing, vec!["artifacts".to_string()]);
        assert!(!deviation.is_empty(), "it deviates in the other direction");
    }

    #[test]
    fn a_torn_final_line_is_skipped_and_a_malformed_complete_line_errors() {
        let dir = tempfile::tempdir().unwrap();
        let good = complete_line(Kind::Note, 1);
        let path = dir.path().join("ledger.jsonl");
        std::fs::write(&path, format!("{good}\n{{\"schema\":\"selfhost.led")).unwrap();
        assert_eq!(read_all(&path, Reader::Strict).unwrap().len(), 1);

        std::fs::write(&path, format!("{good}\nnot json at all\n")).unwrap();
        match read_all(&path, Reader::Strict) {
            Err(ReadError::Fault(_)) => {}
            other => panic!("a malformed complete line must error, got {other:?}"),
        }
    }

    #[test]
    fn append_is_the_writer_and_seq_increases_strictly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("ledger.jsonl");
        let mut fields = null_fields(Kind::Note);
        fields.insert("text".into(), Value::String("one".into()));
        fields.insert("artifacts".into(), Value::Array(vec![]));
        append(&path, Kind::Note, 1, Some("exp-t"), fields.clone()).unwrap();
        let records = read_all(&path, Reader::Strict).unwrap();
        assert_eq!(next_seq(&records), 2);
        append(&path, Kind::Note, next_seq(&records), Some("exp-t"), fields).unwrap();
        let records = read_all(&path, Reader::Strict).unwrap();
        assert_eq!(records.len(), 2);
        assert!(records[1].seq > records[0].seq);
        // and what `append` wrote deviates from nothing, under either reader
        assert!(records.iter().all(|r| r.deviation().is_empty()));
    }

    #[test]
    fn append_refuses_a_record_whose_key_set_is_not_its_kinds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.jsonl");
        // missing `artifacts`
        let mut short = Map::new();
        short.insert("text".into(), Value::String("x".into()));
        assert!(append(&path, Kind::Note, 1, None, short).is_err());
        // and one key too many
        let mut long = null_fields(Kind::Note);
        long.insert("event".into(), Value::String("cycle_closed".into()));
        assert!(append(&path, Kind::Note, 1, None, long).is_err());
        // neither attempt created the file
        assert!(!path.exists() || read_all(&path, Reader::Strict).unwrap().is_empty());
    }

    #[test]
    fn append_refuses_to_have_its_envelope_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.jsonl");
        let mut fields = null_fields(Kind::Note);
        fields.insert("text".into(), Value::String("x".into()));
        fields.insert("artifacts".into(), Value::Array(vec![]));
        fields.insert("seq".into(), Value::from(99));
        assert!(append(&path, Kind::Note, 1, None, fields).is_err());
    }

    #[test]
    fn the_reader_names_itself_for_the_terminal_records_basis() {
        assert_eq!(Reader::Strict.as_str(), "strict");
        assert_eq!(Reader::Phase1Lenient.as_str(), "phase1-lenient");
    }
}
