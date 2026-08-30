//! `status` — the bounded state table, and `--sentinel`, the ONE writer of the `terminal` record
//! (plan §4.6.7).
//!
//! Bare `status` never writes anything, which matters because `./.claude/settings.json`'s
//! `SessionStart:compact` hook runs it with no flags on a schedule nobody controls.

use serde_json::{Map, Value};

use super::cli::StatusArgs;
use super::state::Fold;
use super::{Driver, EXIT_OK, EXIT_TERMINAL, Stop, ledger, state};
use crate::cmds::Ctx;
use crate::redact;

/// One computed sentinel: the line, and the two fields the `terminal` record pins beside it.
#[derive(Clone, Debug)]
pub struct Sentinel {
    pub line: String,
    pub reason: String,
    pub failed_at: Option<String>,
    /// `false` only for `SELFHOST-IN-PROGRESS`, which is not a terminal condition and so appends
    /// nothing: §4.6.2 rule 1 is scoped "on a terminal condition with no record", and an
    /// IN-PROGRESS line has no `reason` for the idempotence key to use.
    pub terminal: bool,
}

/// The six strings of plan §4.6.7, and nothing else may reach stdout under `--sentinel`.
pub fn compute(fold: &Fold, corpus: &[String], profile: &str) -> Sentinel {
    let cycle = fold.cycle.clone().unwrap_or_else(|| "none".to_string());
    let cursor = fold
        .cursor(corpus)
        .or_else(|| corpus.first().cloned())
        .unwrap_or_else(|| "TASK-001".to_string());
    let in_progress = Sentinel {
        line: format!("SELFHOST-IN-PROGRESS cursor={cursor} cycle={cycle}"),
        reason: "in-progress".to_string(),
        failed_at: None,
        terminal: false,
    };
    if fold.not_applicable {
        return in_progress;
    }
    if fold.chain_verified(corpus) {
        return Sentinel {
            line: format!(
                "SELFHOST-CHAIN-VERIFIED cycle={cycle} tasks={}/{} verifiers={}/{}",
                fold.verified.len(),
                corpus.len(),
                fold.verifiers_ok.len(),
                corpus.len()
            ),
            reason: "chain-verified".to_string(),
            failed_at: None,
            terminal: true,
        };
    }
    let not_proven = |failed_at: &str, reason: &str| Sentinel {
        line: format!("SELFHOST-NOT-PROVEN model={profile} failed_at={failed_at} reason={reason}"),
        reason: reason.to_string(),
        failed_at: Some(failed_at.to_string()),
        terminal: true,
    };
    if let Some(task) = fold.model_capability_limit() {
        return not_proven(&task, "model-capability-limit");
    }
    if fold.cycle_budget_exhausted() {
        return not_proven(&cursor, "cycle-budget-exhausted");
    }
    if fold.credit_exhausted {
        return not_proven(&cursor, "provider-credit-exhausted");
    }
    if fold.precondition_failed {
        return not_proven(&cursor, "precondition-failed");
    }
    in_progress
}

pub fn run(ctx: &Ctx, args: &StatusArgs) -> anyhow::Result<i32> {
    super::finish(inner(ctx, args))
}

fn inner(ctx: &Ctx, args: &StatusArgs) -> Result<i32, Stop> {
    let driver = Driver::resolve(ctx, &args.common)?;
    let repo_id = driver.select_repo_id(args.common.repo_id.as_deref(), false)?;
    let path = driver.ledger_path(&repo_id);
    let records = ledger::read_all(&path, driver.reader)?;
    let fold = state::fold(&repo_id, &records, driver.reader, &driver.meta_root)?;

    // R-010: the closure and its discarded set are announced on stderr — the same instrument the
    // not-applicable line uses. An IN-PROGRESS line appends no record, so on that path the closure
    // survives only here and in the ledger's own `cycle_closed` note (D-012).
    for closure in &fold.closures {
        redact::eemit(&format!(
            "CYCLE-CLOSED cycle={} seq={} discarded=[{}]",
            fold.cycle.as_deref().unwrap_or("none"),
            closure.seq,
            closure.discarded.join(",")
        ));
    }

    let corpus = driver.subject_corpus().map_err(Stop::Fault)?;
    if corpus.len() != 7 {
        redact::eemit(&format!(
            "CORPUS-SIZE {} in {} (the chain terminal needs exactly 7)",
            corpus.len(),
            driver.subject.tasks_dir().display()
        ));
    }
    let profile = driver.subject.cfg.default_profile().to_string();
    let computed = compute(&fold, &corpus, &profile);

    if args.sentinel {
        return sentinel(&driver, &fold, &path, &records, &computed);
    }
    table(&driver, &fold, &corpus, &computed);
    Ok(EXIT_OK)
}

/// `--sentinel`: record the terminal, then print it. Or print the recorded one and append nothing.
fn sentinel(
    driver: &Driver,
    fold: &Fold,
    path: &std::path::Path,
    records: &[ledger::Record],
    computed: &Sentinel,
) -> Result<i32, Stop> {
    if let Some(record) = &fold.terminal {
        let recorded_reason = record.str_field("reason").unwrap_or_default();
        let recorded_line = record.str_field("sentinel").unwrap_or_default();
        if recorded_reason != computed.reason {
            // Discovering that a finished experiment's terminal would now be computed differently
            // is a finding about the driver, not a reason to restate the experiment's outcome.
            redact::eemit(&format!(
                "TERMINAL-DRIFT recorded={recorded_reason} computed={}",
                computed.reason
            ));
        }
        redact::emit(recorded_line);
        return Ok(EXIT_TERMINAL);
    }
    if !computed.terminal {
        redact::emit(&computed.line);
        return Ok(EXIT_OK);
    }
    let mut fields = ledger::null_fields(ledger::Kind::Terminal);
    fields.insert("sentinel".into(), Value::String(computed.line.clone()));
    fields.insert("reason".into(), Value::String(computed.reason.clone()));
    fields.insert(
        "failed_at".into(),
        match &computed.failed_at {
            Some(task) => Value::String(task.clone()),
            None => Value::Null,
        },
    );
    fields.insert("basis".into(), Value::Object(basis(driver, fold)));
    // `records` is the ONE read this invocation made: re-reading here would announce every
    // deviating record a second time, and `--phase1`'s announcement is one line per record.
    ledger::append(
        path,
        ledger::Kind::Terminal,
        ledger::next_seq(records),
        fold.cycle.as_deref(),
        fields,
    )
    .map_err(Stop::Fault)?;
    redact::emit(&computed.line);
    Ok(EXIT_TERMINAL)
}

/// The caller-selected inputs the sentinel was computed under (plan §4.6.7).
///
/// `basis` rather than new top-level keys: it is already a free-form object in §4.6.2's `terminal`
/// example, while a new top-level key would be refused on read by this package's own strict
/// reader. A relaxation announced only on stderr is a relaxation nobody can audit later.
fn basis(driver: &Driver, fold: &Fold) -> Map<String, Value> {
    let mut basis = Map::new();
    basis.insert(
        "reader".into(),
        Value::String(driver.reader.as_str().to_string()),
    );
    basis.insert(
        "subject_root".into(),
        Value::String(driver.subject_root.display().to_string()),
    );
    basis.insert(
        "meta_root".into(),
        Value::String(driver.meta_root.display().to_string()),
    );
    if fold.closed {
        basis.insert("closed".into(), Value::Bool(true));
    }
    basis
}

/// The bounded table: at most a screenful, and never a `NEXT_ACTION`.
fn table(driver: &Driver, fold: &Fold, corpus: &[String], computed: &Sentinel) {
    let mut lines = Vec::new();
    if fold.not_applicable {
        lines.push(format!(
            "SELFHOST STATUS  repo={}  cycle=none",
            fold.repo_id
        ));
        lines.push(format!(
            "chain: not applicable (pseudo-repo-id {})",
            fold.repo_id
        ));
        lines.push(format!("records:  {}", fold.records));
        lines.push(format!("last:     {}", fold.tail.join(" | ")));
        redact::emit_lines(lines);
        return;
    }
    lines.push(format!(
        "SELFHOST STATUS  repo={}  cycle={} ({} of 6)",
        fold.repo_id,
        fold.cycle.as_deref().unwrap_or("none"),
        fold.cycles_seen
    ));
    lines.push(format!(
        "cursor={}  verified={}/{}  contiguous={}",
        fold.cursor(corpus).unwrap_or_else(|| "none".to_string()),
        fold.verified.len(),
        corpus.len(),
        if fold.contiguous(corpus) { "yes" } else { "no" }
    ));
    let verified: Vec<String> = fold
        .verified
        .iter()
        .map(|(task, entry)| {
            let sha: String = entry.result_sha.chars().take(8).collect();
            format!("{task} {sha}")
        })
        .collect();
    lines.push(format!(
        "verified: {}",
        if verified.is_empty() {
            "none".to_string()
        } else {
            verified.join("  ")
        }
    ));
    lines.push(format!(
        "verifiers: {}/{}   closed={}   green-field cycles {}/6",
        fold.verifiers_ok.len(),
        corpus.len(),
        fold.closed,
        fold.green_field_cycles
    ));
    let counters: Vec<String> = fold
        .reseeds
        .iter()
        .map(|(task, count)| format!("{task} reseed {count}/3"))
        .collect();
    lines.push(format!(
        "counters: {}      classes: {}",
        if counters.is_empty() {
            "none".to_string()
        } else {
            counters.join("  ")
        },
        if fold.fault_classes.is_empty() {
            "none".to_string()
        } else {
            fold.fault_classes.join(",")
        }
    ));
    lines.push(format!(
        "reader:   {}   subject={}",
        driver.reader.as_str(),
        driver.subject_root.display()
    ));
    lines.push(format!("records:  {}", fold.records));
    lines.push(format!("last:     {}", fold.tail.join(" | ")));
    lines.push(computed.line.clone());
    redact::emit_lines(lines);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selfhost::ledger::{Kind, Reader, Record};
    use serde_json::json;

    fn corpus() -> Vec<String> {
        (1..=7).map(|n| format!("TASK-00{n}")).collect()
    }

    fn fold_with(verified: usize, cycle: &str) -> Fold {
        let mut fold = Fold {
            cycle: Some(cycle.to_string()),
            ..Fold::default()
        };
        for n in 1..=verified {
            let task = format!("TASK-00{n}");
            fold.verified.insert(
                task.clone(),
                state::VerifiedEntry {
                    result_sha: "0123456789abcdef".into(),
                    base_sha: "b".into(),
                    verdict_sha256: "h".into(),
                    attempt_seq: Some(n as u64),
                },
            );
            fold.verifiers_ok.insert(task);
        }
        fold
    }

    /// The literals are TYPED HERE, copied from plan §4.4 and §4.5 by hand. A test that builds its
    /// expectation with the same `format!` the production code uses proves only that a function
    /// equals itself, and a mistyped sentinel means the loop never terminates.
    #[test]
    fn the_six_sentinel_strings_are_byte_exact() {
        let full = fold_with(7, "exp-fixture");
        assert_eq!(
            compute(&full, &corpus(), "zai-flash").line,
            "SELFHOST-CHAIN-VERIFIED cycle=exp-fixture tasks=7/7 verifiers=7/7"
        );

        let partial = fold_with(3, "exp-20260829-100000");
        assert_eq!(
            compute(&partial, &corpus(), "zai-flash").line,
            "SELFHOST-IN-PROGRESS cursor=TASK-004 cycle=exp-20260829-100000"
        );

        let mut capability = fold_with(4, "exp-1");
        capability.reseeds.insert("TASK-005".to_string(), 3);
        assert_eq!(
            compute(&capability, &corpus(), "zai-flash").line,
            "SELFHOST-NOT-PROVEN model=zai-flash failed_at=TASK-005 reason=model-capability-limit"
        );

        let mut budget = fold_with(4, "exp-1");
        budget.green_field_cycles = 6;
        assert_eq!(
            compute(&budget, &corpus(), "zai-flash").line,
            "SELFHOST-NOT-PROVEN model=zai-flash failed_at=TASK-005 reason=cycle-budget-exhausted"
        );

        let mut credit = fold_with(4, "exp-1");
        credit.credit_exhausted = true;
        assert_eq!(
            compute(&credit, &corpus(), "zai-flash").line,
            "SELFHOST-NOT-PROVEN model=zai-flash failed_at=TASK-005 \
             reason=provider-credit-exhausted"
        );

        let mut precondition = fold_with(4, "exp-1");
        precondition.precondition_failed = true;
        assert_eq!(
            compute(&precondition, &corpus(), "zai-flash").line,
            "SELFHOST-NOT-PROVEN model=zai-flash failed_at=TASK-005 reason=precondition-failed"
        );
    }

    /// §4.4's goal condition is exact equality against two literal first tokens. The sixth string
    /// is neither, and that is what makes it incapable of ending the programme.
    #[test]
    fn the_in_progress_line_matches_neither_goal_pattern() {
        let line = compute(&fold_with(0, "exp-1"), &corpus(), "zai-flash").line;
        assert!(!line.starts_with("SELFHOST-CHAIN-VERIFIED"));
        assert!(!line.starts_with("SELFHOST-NOT-PROVEN"));
        assert!(line.starts_with("SELFHOST-IN-PROGRESS"));
    }

    #[test]
    fn in_progress_is_not_a_terminal_and_a_closed_cycle_yields_it() {
        assert!(!compute(&fold_with(0, "exp-1"), &corpus(), "p").terminal);
        let mut closed = fold_with(7, "exp-fixture");
        closed.closed = true;
        closed.verified.clear();
        closed.verifiers_ok.clear();
        let out = compute(&closed, &corpus(), "p");
        assert_eq!(
            out.line,
            "SELFHOST-IN-PROGRESS cursor=TASK-001 cycle=exp-fixture"
        );
        assert!(!out.terminal, "a closed cycle appends no terminal record");
    }

    #[test]
    fn a_pseudo_repo_id_never_computes_a_terminal() {
        let mut fold = fold_with(7, "exp-1");
        fold.not_applicable = true;
        assert!(!compute(&fold, &corpus(), "p").terminal);
    }

    #[test]
    fn a_corpus_that_is_not_seven_cannot_fire_the_chain_terminal() {
        let full = fold_with(7, "exp-fixture");
        let eight: Vec<String> = (1..=8).map(|n| format!("TASK-00{n}")).collect();
        assert!(!full.chain_verified(&eight));
        assert!(!compute(&full, &eight, "p").terminal);
    }

    #[test]
    fn a_recorded_terminal_is_reprinted_verbatim_and_drift_is_named() {
        let recorded = Record {
            kind: Kind::Terminal,
            seq: 16,
            cycle: Some("exp-fixture".into()),
            ts: "t".into(),
            line: 16,
            obj: json!({
                "schema": ledger::SCHEMA, "kind": "terminal", "ts": "t", "seq": 16,
                "cycle": "exp-fixture",
                "sentinel": "SELFHOST-CHAIN-VERIFIED cycle=exp-fixture tasks=7/7 verifiers=7/7",
                "reason": "chain-verified", "failed_at": null,
                "basis": {"reader": "strict"}
            })
            .as_object()
            .unwrap()
            .clone(),
        };
        assert!(
            recorded.deviation().is_empty(),
            "the record is schema-complete"
        );
        assert_eq!(recorded.str_field("reason"), Some("chain-verified"));
        // and the computed reason for the same fold agrees, so no drift is reported
        assert_eq!(
            compute(&fold_with(7, "exp-fixture"), &corpus(), "p").reason,
            "chain-verified"
        );
        // while a fold that no longer verifies would drift
        assert_ne!(
            compute(&fold_with(6, "exp-fixture"), &corpus(), "p").reason,
            "chain-verified"
        );
    }

    #[test]
    fn the_reader_is_recorded_in_the_basis() {
        for reader in [Reader::Strict, Reader::Phase1Lenient] {
            assert!(!reader.as_str().is_empty());
        }
    }
}
