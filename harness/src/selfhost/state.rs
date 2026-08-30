//! The fold: `cursor`, `verified{}`, the counters, and the five terminal conditions (plan §4.6.4,
//! §4.6.7).
//!
//! **Nothing is stored.** Everything here is folded out of the ledger on every call, in `seq`
//! order, which is what makes a stale session harmless: the loop can be killed at any instant and
//! the next read recomputes the truth.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;

use super::hash;
use super::ledger::{Kind, Reader, Record};
use super::{Driver, Stop};

/// What a chain task's `verified{}` entry holds (plan §3.3 plus §4.6.19 item 9's `attempt_seq`).
#[derive(Clone, Debug)]
pub struct VerifiedEntry {
    pub result_sha: String,
    pub base_sha: String,
    pub verdict_sha256: String,
    pub attempt_seq: Option<u64>,
}

/// A cycle closure the relaxed reader honoured: which record said so and what it discarded.
#[derive(Clone, Debug)]
pub struct Closure {
    pub seq: u64,
    pub discarded: Vec<String>,
}

/// The folded state of one ledger.
#[derive(Clone, Debug, Default)]
pub struct Fold {
    pub repo_id: String,
    pub cycle: Option<String>,
    pub records: usize,
    pub cycles_seen: usize,
    pub green_field_cycles: usize,
    pub verified: BTreeMap<String, VerifiedEntry>,
    pub verifiers_ok: BTreeSet<String>,
    pub closed: bool,
    pub closures: Vec<Closure>,
    /// A ledger under a reserved pseudo-repo-id: no chain, no cursor, by construction.
    pub not_applicable: bool,
    /// The `terminal` record of the open cycle, when one has been written.
    pub terminal: Option<Record>,
    pub reseeds: BTreeMap<String, usize>,
    pub dispatches: BTreeMap<String, usize>,
    pub fault_classes: Vec<String>,
    pub probe_failed: bool,
    pub credit_exhausted: bool,
    pub precondition_failed: bool,
    pub tail: Vec<String>,
}

/// The `attempt` record's promotability, read exactly as `cmds::status::is_promotable` reads it
/// (`harness/src/cmds/status.rs:425-431`): `GOAL_MET` carries an evaluator verdict by
/// construction; `IDLE` and `GOAL_CLEARED_ERROR` do not, and need the completion evidence
/// `Status::completion_evidence` names (`:54-57`) — a real verdict, or the agent's own
/// `GOAL_RESULT` line. A record that carries neither field establishes neither.
fn promotable(attempt: &Record) -> bool {
    match attempt.str_field("status_state").unwrap_or_default() {
        crate::cmds::status::GOAL_MET => true,
        crate::cmds::status::IDLE | crate::cmds::status::GOAL_CLEARED_ERROR => {
            attempt
                .u64_field("goal_verdicts")
                .is_some_and(|count| count >= 1)
                || attempt
                    .str_field("goal_result_line")
                    .is_some_and(|line| !line.trim().is_empty())
        }
        _ => false,
    }
}

fn gate_passed(attempt: &Record) -> bool {
    attempt
        .get("gate")
        .and_then(|gate| gate.get("verdict"))
        .and_then(Value::as_str)
        == Some("pass")
}

/// The corpus a `verdict` record belongs to.
///
/// One of TASK-114 D-005's two normalizations, and it is available to the relaxed reader only: an
/// ABSENT `corpus` reads as `experiment` when `verdict_file` sits under the ledger's own repo id,
/// which is how plan §4.6.4 says the corpus must be decided — "by where the file lives, never
/// inferred from the id".
fn corpus_of(verdict: &Record, repo_id: &str, reader: Reader) -> Option<String> {
    if let Some(corpus) = verdict.str_field("corpus") {
        return Some(corpus.to_string());
    }
    if reader != Reader::Phase1Lenient {
        return None;
    }
    let file = verdict.str_field("verdict_file")?;
    let home = format!("selfhost/state/{repo_id}/");
    file.starts_with(&home).then(|| "experiment".to_string())
}

/// Plan §3.4 condition 3, read from the record and never re-run.
///
/// The second of D-005's two normalizations: `condition3` reads as `gh_check` when `gh_check` is
/// absent or null, with `ok = (http == 200) && (observed is absent || observed == pushed_sha)`.
fn gh_check_ok(verdict: &Record, reader: Reader) -> bool {
    if let Some(check) = verdict.get("gh_check") {
        return check.get("ok").and_then(Value::as_bool) == Some(true);
    }
    if reader != Reader::Phase1Lenient {
        return false;
    }
    let Some(legacy) = verdict.get("condition3") else {
        return false;
    };
    let http_ok = legacy.get("http").and_then(Value::as_u64) == Some(200);
    let observed_ok = match legacy.get("observed").and_then(Value::as_str) {
        None => true,
        Some(observed) => Some(observed) == verdict.str_field("pushed_sha"),
    };
    http_ok && observed_ok
}

/// The `attempt` record a `verdict` refers to.
///
/// `attempt_seq` is a pointer to a record's `seq` and is followed when present. The hand-kept
/// Phase 1 spelling `attempt` is an ORDINAL and is **not** mapped to it (D-005): the committed
/// 192730 verdict carries `"attempt": 1` while the attempt record it refers to is `"seq": 2`, so
/// mapping 1 to 1 would point the fold at the `cycle_open`. When the pointer is absent the
/// relaxed reader binds by `(cycle, task)` to the newest attempt below the verdict instead —
/// without which the relaxed fold could never produce a non-empty `verified{}` and plan §4.6.19
/// item 25's whole hazard ("true of the file and false of the chain") would be unreachable.
fn bound_attempt<'a>(
    verdict: &Record,
    records: &'a [Record],
    reader: Reader,
) -> Option<&'a Record> {
    if let Some(seq) = verdict.u64_field("attempt_seq") {
        return records
            .iter()
            .find(|record| record.kind == Kind::Attempt && record.seq == seq);
    }
    if reader != Reader::Phase1Lenient {
        return None;
    }
    let task = verdict.str_field("task")?;
    records.iter().rev().find(|record| {
        record.kind == Kind::Attempt
            && record.seq < verdict.seq
            && record.cycle == verdict.cycle
            && record.str_field("task") == Some(task)
    })
}

/// Fold one ledger's records.
///
/// `verdict_root` is the directory verdict-file paths resolve against: this repository's root,
/// because `verdict_file` is stored relative to it and an absolute path would pin the ledger to
/// one laptop.
pub fn fold(
    repo_id: &str,
    records: &[Record],
    reader: Reader,
    verdict_root: &Path,
) -> Result<Fold, Stop> {
    let mut out = Fold {
        repo_id: repo_id.to_string(),
        records: records.len(),
        ..Fold::default()
    };
    out.tail = records
        .iter()
        .rev()
        .take(3)
        .map(|record| format!("seq {} {}", record.seq, record.kind.as_str()))
        .collect();

    // across the WHOLE file, per §4.6.7's cycle-budget row
    for record in records {
        if record.kind == Kind::CycleOpen {
            out.cycles_seen += 1;
            if record.str_field("reason") == Some("green-field") {
                out.green_field_cycles += 1;
            }
        }
    }

    if Driver::is_pseudo(repo_id) {
        // §4.6.4: the fold is refused outright for a reserved pseudo-repo-id — `verified` is
        // empty and there is no cursor, which is what keeps §3.3's three reasons intact.
        out.not_applicable = true;
        return Ok(out);
    }

    out.cycle = records
        .iter()
        .rev()
        .find(|record| record.kind == Kind::CycleOpen)
        .and_then(|record| record.cycle.clone())
        .or_else(|| records.last().and_then(|record| record.cycle.clone()));

    let open: Vec<&Record> = records
        .iter()
        .filter(|record| record.cycle == out.cycle)
        .collect();

    for record in &open {
        match record.kind {
            Kind::CycleOpen => {
                out.verified.clear();
                out.verifiers_ok.clear();
                out.closed = false;
            }
            Kind::Verdict => {
                let admission = admit(record, records, repo_id, reader, verdict_root)?;
                if let Some(task) = record.str_field("task") {
                    if admission.file_verified {
                        out.verifiers_ok.insert(task.to_string());
                    }
                    if let Some(entry) = admission.entry {
                        out.verified.insert(task.to_string(), entry);
                    }
                }
            }
            Kind::Revoke => {
                if let Some(from) = record.str_field("earliest_bad_task") {
                    out.verified.retain(|task, _| task.as_str() < from);
                    out.verifiers_ok.retain(|task| task.as_str() < from);
                }
            }
            Kind::Reset => {
                if let Some(to) = record.str_field("to_task") {
                    out.verified.retain(|task, _| task.as_str() <= to);
                    out.verifiers_ok.retain(|task| task.as_str() <= to);
                }
            }
            Kind::Terminal => out.terminal = Some((*record).clone()),
            Kind::EnvFault => {
                let terminal = record.bool_field("terminal") == Some(true);
                match record.str_field("class").unwrap_or_default() {
                    "credit-exhausted" if terminal => out.credit_exhausted = true,
                    "precondition" if terminal => out.precondition_failed = true,
                    _ => {}
                }
                if record.str_field("source") == Some("probe") {
                    out.probe_failed = true;
                }
            }
            Kind::DispatchIntent => {
                if let Some(task) = record.str_field("task") {
                    *out.dispatches.entry(task.to_string()).or_default() += 1;
                }
            }
            // The ONE note semantic the relaxed reader takes, and no other (§4.6.19 item 25).
            // Under the strict reader `event` is an unknown top-level key and the record is
            // refused before it ever reaches here, which is what makes the exception
            // self-limiting.
            Kind::Note
                if reader == Reader::Phase1Lenient
                    && record.str_field("event") == Some("cycle_closed") =>
            {
                let discarded = record
                    .get("discarded_verified")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_else(|| out.verified.keys().cloned().collect());
                out.closures.push(Closure {
                    seq: record.seq,
                    discarded,
                });
                // Closure drops `verified` to empty exactly as `cycle_open` does, so the cursor
                // returns to TASK-001 (R-010, D-012).
                out.verified.clear();
                out.verifiers_ok.clear();
                out.closed = true;
            }
            _ => {}
        }
    }

    // reseed_counter: attempts whose BOUND diagnosis is `agent-work` (§4.6.3, §4.6.4)
    for record in &open {
        if record.kind != Kind::Attempt {
            continue;
        }
        let Some(task) = record.str_field("task") else {
            continue;
        };
        let bound = open
            .iter()
            .filter(|other| other.kind == Kind::Diagnosis)
            .filter(|other| other.u64_field("attempt_seq") == Some(record.seq))
            .max_by_key(|other| other.seq);
        if let Some(diagnosis) = bound {
            let class = diagnosis.str_field("fault_class").unwrap_or_default();
            out.fault_classes.push(class.to_string());
            if class == "agent-work" {
                *out.reseeds.entry(task.to_string()).or_default() += 1;
            }
        }
    }
    Ok(out)
}

/// What one `verdict` record contributes: its `verified{}` entry when §3.4's three conditions
/// re-check clean, and — separately — whether its verdict FILE is on disk and hashes to what the
/// ledger says. `verifiers=M/7` counts the second (R-009); `tasks=N/7` counts the first.
struct Admission {
    entry: Option<VerifiedEntry>,
    file_verified: bool,
}

/// §3.4's three conditions, re-checked from the ledger on every fold rather than trusted.
fn admit(
    verdict: &Record,
    records: &[Record],
    repo_id: &str,
    reader: Reader,
    verdict_root: &Path,
) -> Result<Admission, Stop> {
    // The verdict file's bytes are checked whatever the corpus: a present file whose digest
    // differs means the artifact of record no longer matches the record, and counting that as a
    // lower verifier count would report a partial chain where the evidence moved (D-007).
    // §3.4 condition 2. An ABSENT file does not retro-invalidate the entry — it is committed, so
    // it normally is present — but a PRESENT file whose digest differs means the artifact of
    // record no longer matches the record, and that refuses rather than lowering a count (D-007).
    let recorded = verdict.str_field("verdict_sha256");
    let mut file_of_record = false;
    if let (Some(relative), Some(recorded)) = (verdict.str_field("verdict_file"), recorded) {
        file_of_record = true;
        let path = verdict_root.join(relative);
        if path.is_file() {
            let observed = hash::digest_file(&path).map_err(Stop::Fault)?;
            if observed != recorded {
                return Err(Stop::Refused(format!(
                    "LEDGER-REFUSED verdict-tampered {relative} seq={} recorded={recorded} \
                     observed={observed}",
                    verdict.seq
                )));
            }
        }
    }
    let experiment = corpus_of(verdict, repo_id, reader).as_deref() == Some("experiment");
    let none = Admission {
        entry: None,
        file_verified: file_of_record && experiment,
    };
    if !experiment {
        return Ok(none);
    }
    if verdict.str_field("overall") != Some("PASS") {
        return Ok(none);
    }
    if !gh_check_ok(verdict, reader) {
        return Ok(none);
    }
    let Some(attempt) = bound_attempt(verdict, records, reader) else {
        return Ok(none);
    };
    if !gate_passed(attempt) || !promotable(attempt) {
        return Ok(none);
    }
    Ok(Admission {
        file_verified: none.file_verified,
        entry: Some(VerifiedEntry {
            result_sha: attempt
                .str_field("result_sha")
                .or_else(|| verdict.str_field("pushed_sha"))
                .unwrap_or_default()
                .to_string(),
            base_sha: verdict
                .str_field("base_sha")
                .unwrap_or_default()
                .to_string(),
            verdict_sha256: recorded.unwrap_or_default().to_string(),
            attempt_seq: Some(attempt.seq),
        }),
    })
}

impl Fold {
    /// The lowest corpus id absent from `verified{}`; `None` when every one is verified.
    pub fn cursor(&self, corpus: &[String]) -> Option<String> {
        corpus
            .iter()
            .find(|task| !self.verified.contains_key(*task))
            .cloned()
    }

    /// A chain only when contiguous from the first corpus id.
    pub fn contiguous(&self, corpus: &[String]) -> bool {
        let verified = self.verified.len();
        corpus
            .iter()
            .take(verified)
            .all(|task| self.verified.contains_key(task))
    }

    /// `SELFHOST-CHAIN-VERIFIED`'s condition (§4.6.7), plus R-010's: a cycle the fold has seen
    /// closed can never yield it.
    pub fn chain_verified(&self, corpus: &[String]) -> bool {
        !self.closed
            && corpus.len() == 7
            && self.verified.len() == 7
            && self.contiguous(corpus)
            && self.verifiers_ok.len() == 7
    }

    /// The task whose `reseed_counter` reached 3, if the Z.ai conjunct also holds.
    ///
    /// §7.0's conjunct is not optional: three reseeds against a dead credential produce three
    /// identical `agent-work` diagnoses and walk into the terminal, which is indistinguishable
    /// from the true finding in the ledger. `probe` is a successor's subcommand (R-013) and writes
    /// an `env_fault` only on failure, so the conjunct is read here as "no probe fault recorded".
    pub fn model_capability_limit(&self) -> Option<String> {
        if self.probe_failed {
            return None;
        }
        self.reseeds
            .iter()
            .find(|(_, count)| **count >= 3)
            .map(|(task, _)| task.clone())
    }

    /// Six green-field `cycle_open` records across the whole ledger (§4.5's six-cycle bound).
    pub fn cycle_budget_exhausted(&self) -> bool {
        self.green_field_cycles >= 6
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selfhost::ledger;
    use serde_json::json;

    fn record(value: serde_json::Value) -> Record {
        let obj = value.as_object().unwrap().clone();
        Record {
            kind: Kind::parse(obj["kind"].as_str().unwrap()).unwrap(),
            seq: obj["seq"].as_u64().unwrap(),
            cycle: obj["cycle"].as_str().map(str::to_string),
            ts: String::new(),
            line: 0,
            obj,
        }
    }

    fn corpus() -> Vec<String> {
        (1..=7).map(|n| format!("TASK-00{n}")).collect()
    }

    /// A cycle_open plus `n` attempt/verdict pairs, every record schema-complete.
    fn chain(n: usize) -> Vec<Record> {
        let mut out = vec![record(json!({
            "schema": ledger::SCHEMA, "kind": "cycle_open", "ts": "t", "seq": 1,
            "cycle": "exp-t", "repo_url": null, "repo_id": "repo-x", "experiment_id": "exp-t",
            "bootstrap_sha": null, "reason": "first", "green_field_cause": "first",
            "predecessor_cycle": null, "discarded_verified": [], "harness_sha": null,
            "selfhost_sha": null, "window_start": "t", "window_end": null
        }))];
        let mut seq = 1;
        for i in 1..=n {
            let task = format!("TASK-00{i}");
            seq += 1;
            let attempt_seq = seq;
            out.push(record(json!({
                "schema": ledger::SCHEMA, "kind": "attempt", "ts": "t", "seq": seq,
                "cycle": "exp-t", "task": task.clone(), "run_id": "r", "run_dir": "d",
                "container": "c", "base_sha": "b", "result_sha": format!("sha-{i}"),
                "gate": {"verdict": "pass", "exit": 0, "last_line": "DONE", "head": "h"},
                "status_state": "GOAL_MET", "started": "t", "finished": "t",
                "verdict_file": null, "verdict_sha256": null, "agent": "a", "model": "m",
                "effort": "high", "operator_note": null, "tuple": {}, "intent_seq": null
            })));
            seq += 1;
            out.push(record(json!({
                "schema": ledger::SCHEMA, "kind": "verdict", "ts": "t", "seq": seq,
                "cycle": "exp-t", "task": task.clone(), "corpus": "experiment", "fixture": null,
                "attempt_seq": attempt_seq,
                "verdict_file": format!("selfhost/state/repo-x/verdicts/{task}-1.json"),
                "verdict_sha256": hash::digest_bytes(task.as_bytes()),
                "overall": "PASS", "verifier": "verifier", "pushed_sha": format!("sha-{i}"),
                "base_sha": "b", "gh_check": {"ok": true}, "counts": {}, "extra_keys": [],
                "decisive_evidence": null, "diagnosis_id": null
            })));
        }
        out
    }

    fn folded(records: &[Record], reader: Reader) -> Fold {
        let dir = tempfile::tempdir().unwrap();
        fold("repo-x", records, reader, dir.path()).unwrap()
    }

    #[test]
    fn seven_of_seven_is_a_verified_chain_and_six_is_not() {
        let full = folded(&chain(7), Reader::Strict);
        assert_eq!(full.verified.len(), 7);
        assert!(full.contiguous(&corpus()));
        assert!(full.chain_verified(&corpus()));
        assert_eq!(full.cursor(&corpus()), None);

        let short = folded(&chain(6), Reader::Strict);
        assert!(!short.chain_verified(&corpus()));
        assert_eq!(short.cursor(&corpus()), Some("TASK-007".to_string()));
    }

    #[test]
    fn a_closed_cycle_can_never_yield_the_chain_terminal() {
        let mut records = chain(7);
        records.push(record(json!({
            "schema": ledger::SCHEMA, "kind": "note", "ts": "t", "seq": 99, "cycle": "exp-t",
            "text": "closed", "artifacts": [], "event": "cycle_closed",
            "discarded_verified": ["TASK-001"]
        })));
        let closed = folded(&records, Reader::Phase1Lenient);
        assert!(closed.closed);
        assert!(closed.verified.is_empty(), "closure empties verified{{}}");
        assert!(!closed.chain_verified(&corpus()));
        assert_eq!(closed.cursor(&corpus()), Some("TASK-001".to_string()));
        assert_eq!(closed.closures.len(), 1);
    }

    #[test]
    fn no_other_note_event_closes_anything() {
        let mut records = chain(7);
        records.push(record(json!({
            "schema": ledger::SCHEMA, "kind": "note", "ts": "t", "seq": 99, "cycle": "exp-t",
            "text": "a debate round closed", "artifacts": [], "event": "debate_round_closed"
        })));
        let open = folded(&records, Reader::Phase1Lenient);
        assert!(!open.closed, "only event=cycle_closed closes a cycle");
        assert!(open.chain_verified(&corpus()));
    }

    #[test]
    fn the_strict_reader_takes_no_note_semantics_at_all() {
        let mut records = chain(7);
        records.push(record(json!({
            "schema": ledger::SCHEMA, "kind": "note", "ts": "t", "seq": 99, "cycle": "exp-t",
            "text": "closed", "artifacts": [], "event": "cycle_closed"
        })));
        // (a strict READ would refuse this line outright; the fold ignores the key regardless)
        let strict = folded(&records, Reader::Strict);
        assert!(!strict.closed);
    }

    #[test]
    fn a_non_experiment_corpus_is_recorded_and_folded_into_nothing() {
        let mut records = chain(6);
        records.push(record(json!({
            "schema": ledger::SCHEMA, "kind": "verdict", "ts": "t", "seq": 90, "cycle": "exp-t",
            "task": "TASK-007", "corpus": "calibration", "fixture": "f", "attempt_seq": null,
            "verdict_file": null, "verdict_sha256": null, "overall": "PASS",
            "verifier": "verifier", "pushed_sha": "s", "base_sha": "b", "gh_check": null,
            "counts": {}, "extra_keys": [], "decisive_evidence": null, "diagnosis_id": null
        })));
        let out = folded(&records, Reader::Strict);
        assert_eq!(out.verified.len(), 6, "a calibration verdict never folds");
        assert!(!out.chain_verified(&corpus()));
    }

    #[test]
    fn a_gate_pass_with_a_non_promotable_status_is_not_a_pass() {
        let mut records = chain(1);
        // rewrite the attempt's status_state to a bare IDLE with no completion evidence
        records[1].obj.insert("status_state".into(), json!("IDLE"));
        let out = folded(&records, Reader::Strict);
        assert!(out.verified.is_empty());
        // the same IDLE WITH the evidence `completion_evidence` names does promote
        records[1].obj.insert(
            "goal_result_line".into(),
            json!("GOAL_RESULT task=TASK-001 status=DONE"),
        );
        let out = folded(&records, Reader::Phase1Lenient);
        assert_eq!(out.verified.len(), 1);
    }

    #[test]
    fn the_hand_kept_condition3_and_absent_corpus_normalize_only_under_phase1() {
        let mut records = chain(1);
        let verdict = records.last_mut().unwrap();
        verdict.obj.remove("corpus");
        verdict.obj.remove("gh_check");
        verdict.obj.remove("attempt_seq");
        verdict.obj.insert(
            "verdict_file".into(),
            json!("selfhost/state/repo-x/verdicts/TASK-001-1.json"),
        );
        verdict.obj.insert("attempt".into(), json!(1));
        verdict.obj.insert(
            "condition3".into(),
            json!({"http": 200, "observed": "sha-1", "command": "gh api …"}),
        );
        verdict.obj.insert("pushed_sha".into(), json!("sha-1"));
        assert!(
            folded(&records, Reader::Strict).verified.is_empty(),
            "strict takes no normalization"
        );
        assert_eq!(folded(&records, Reader::Phase1Lenient).verified.len(), 1);
    }

    #[test]
    fn a_moved_verdict_file_refuses_rather_than_lowering_the_count() {
        let dir = tempfile::tempdir().unwrap();
        let verdicts = dir.path().join("selfhost/state/repo-x/verdicts");
        std::fs::create_dir_all(&verdicts).unwrap();
        let file = verdicts.join("TASK-001-1.json");
        std::fs::write(&file, b"{}").unwrap();
        let mut records = chain(1);
        let verdict = records.last_mut().unwrap();
        verdict.obj.insert(
            "verdict_file".into(),
            json!("selfhost/state/repo-x/verdicts/TASK-001-1.json"),
        );
        verdict
            .obj
            .insert("verdict_sha256".into(), json!(hash::digest_bytes(b"{}")));
        assert_eq!(
            fold("repo-x", &records, Reader::Strict, dir.path())
                .unwrap()
                .verified
                .len(),
            1
        );
        std::fs::write(&file, b"{} ").unwrap();
        match fold("repo-x", &records, Reader::Strict, dir.path()) {
            Err(Stop::Refused(msg)) => {
                assert!(msg.starts_with("LEDGER-REFUSED verdict-tampered"), "{msg}")
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_pseudo_repo_id_is_not_folded_at_all() {
        let dir = tempfile::tempdir().unwrap();
        let out = fold("preflight", &chain(7), Reader::Strict, dir.path()).unwrap();
        assert!(out.not_applicable);
        assert!(out.verified.is_empty());
        assert_eq!(out.cursor(&corpus()), Some("TASK-001".to_string()));
    }

    #[test]
    fn a_revoke_drops_verified_from_the_earliest_bad_task_up() {
        let mut records = chain(7);
        records.push(record(json!({
            "schema": ledger::SCHEMA, "kind": "revoke", "ts": "t", "seq": 90, "cycle": "exp-t",
            "earliest_bad_task": "TASK-003", "dropped": ["TASK-003"], "diagnosis_id": "d",
            "reason": "introduced at TASK-003"
        })));
        let out = folded(&records, Reader::Strict);
        assert_eq!(out.verified.len(), 2);
        assert_eq!(out.cursor(&corpus()), Some("TASK-003".to_string()));
    }

    #[test]
    fn reseed_counter_counts_agent_work_bound_diagnoses_and_the_probe_conjunct_gates_it() {
        let mut records = chain(1);
        let attempt_seq = records[1].seq;
        for (seq, class) in [(50, "agent-work"), (51, "agent-work"), (52, "agent-work")] {
            records.push(record(json!({
                "schema": ledger::SCHEMA, "kind": "diagnosis", "ts": "t", "seq": seq,
                "cycle": "exp-t", "diagnosis_id": format!("d{seq}"), "task": "TASK-001",
                "attempt_seq": attempt_seq, "supersedes": null, "framing": "primary",
                "fault_class": class, "earliest_bad_task": "TASK-001", "file": "f",
                "file_sha256": "h", "proposed_paths": [], "replay_scope": "failing-task-only",
                "round": 1, "package_repo": "subject", "package_id": "TASK-001",
                "package_commit": "c", "package_tree_hash": "t", "package_worktree_clean": true,
                "extra_keys": []
            })));
        }
        // one attempt, one BOUND diagnosis (the highest seq), so the counter is 1 and not 3
        let out = folded(&records, Reader::Strict);
        assert_eq!(out.reseeds.get("TASK-001"), Some(&1));
        assert_eq!(out.model_capability_limit(), None);
    }

    #[test]
    fn six_green_field_cycles_exhaust_the_budget() {
        let mut records = Vec::new();
        for seq in 1..=6 {
            records.push(record(json!({
                "schema": ledger::SCHEMA, "kind": "cycle_open", "ts": "t", "seq": seq,
                "cycle": format!("exp-{seq}"), "repo_url": null, "repo_id": "repo-x",
                "experiment_id": "e", "bootstrap_sha": null, "reason": "green-field",
                "green_field_cause": "repair-cap", "predecessor_cycle": null,
                "discarded_verified": [], "harness_sha": null, "selfhost_sha": null,
                "window_start": "t", "window_end": null
            })));
        }
        let out = folded(&records, Reader::Strict);
        assert_eq!(out.green_field_cycles, 6);
        assert!(out.cycle_budget_exhausted());
    }
}
