//! `record` — the two arms this package ships (plan §4.6.6; TASK-114 R-011, D-009).
//!
//! `--verdict-file` with its `--corpus`/`--fixture`/`--preflight` companions, and `--note`. The
//! five arms that are not here — `--attempt`, `--debate`, `--diagnosis`, `--env-fault`,
//! `--revoke`, `--heartbeat` — are R-013's business and a successor's.
//!
//! `record` never emits a `NEXT_ACTION` and never advances anything: the cursor is derived, so
//! "advance the cursor" is not an action anyone can take.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::cli::{Corpus, RecordArgs};
use super::{Driver, EXIT_OK, PREFLIGHT, Stop, hash, ledger};
use crate::cmds::Ctx;
use crate::ops;
use crate::redact;

/// The schema a verdict file must carry.
pub const VERDICT_SCHEMA: &str = "selfhost.verdict/v1";

/// The `selfhost.verdict/v1` top-level keys. Anything else is ACCEPTED, PRESERVED in the record's
/// `extra_keys`, and never rejected — §4.6.2b's EXTERNAL-file rule, which governs this validator
/// and does not govern the ledger's own records.
const VERDICT_KEYS: [&str; 10] = [
    "schema",
    "task",
    "pushed_sha",
    "base_sha",
    "overall",
    "acceptance",
    "requirements",
    "decisions",
    "artifact_runs",
    "decisive_evidence",
];

fn refuse(reason: &str) -> Stop {
    Stop::Refused(format!("VERDICT-REFUSED {reason}"))
}

pub fn run(ctx: &Ctx, args: &RecordArgs) -> anyhow::Result<i32> {
    super::finish(inner(ctx, args))
}

fn inner(ctx: &Ctx, args: &RecordArgs) -> Result<i32, Stop> {
    // The two argument refusals come first, before any tree is read: a run that refuses its own
    // flags must leave the ledger untouched however the rest of the world is arranged.
    if let (Some(path), Some(Corpus::Calibration)) = (&args.verdict_file, args.corpus) {
        let stem = path
            .file_stem()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        match &args.fixture {
            None => {
                return Err(refuse(
                    "fixture-required --corpus calibration names an input, not a chain task",
                ));
            }
            Some(name) if *name != stem => {
                return Err(refuse(&format!(
                    "fixture-name-mismatch --fixture {name} is not the basename {stem}"
                )));
            }
            Some(_) => {}
        }
    }

    let driver = Driver::resolve(ctx, &args.common)?;
    let repo_id = driver.select_repo_id(args.common.repo_id.as_deref(), args.preflight)?;
    let path = driver.ledger_path(&repo_id);
    let records = ledger::read_all(&path, driver.reader)?;
    let seq = ledger::next_seq(&records);
    let cycle = if repo_id == PREFLIGHT {
        None
    } else {
        records
            .iter()
            .rev()
            .find(|record| record.kind == ledger::Kind::CycleOpen)
            .and_then(|record| record.cycle.clone())
    };

    let target = Target {
        repo_id,
        path,
        records,
        seq,
        cycle,
    };
    match (&args.verdict_file, &args.note) {
        (Some(file), _) => verdict(&driver, args, &target, file),
        (None, Some(text)) => note(
            &driver,
            &target.path,
            target.seq,
            target.cycle.as_deref(),
            text,
            &args.artifact,
        ),
        // clap's `required_unless_present` makes this unreachable; it stays fail-closed anyway.
        (None, None) => Err(Stop::Refused(
            "VERDICT-REFUSED no-mode exactly one of --verdict-file or --note".to_string(),
        )),
    }
}

/// `--note "<text>" [--artifact <path>]…` — the only route by which a pre-schema Phase 1 debate
/// round enters the ledger.
///
/// A note is additive, never substitutive: the artifact stays the artifact and the note is an
/// operator statement *about* it, bound to bytes by a hash. `token mint` does not read `note`
/// records, which is what makes it strictly weaker by construction.
fn note(
    driver: &Driver,
    path: &Path,
    seq: u64,
    cycle: Option<&str>,
    text: &str,
    artifacts: &[PathBuf],
) -> Result<i32, Stop> {
    if text.trim().is_empty() {
        return Err(refuse("empty-note --note takes the operator's statement"));
    }
    let mut listed = Vec::new();
    for artifact in artifacts {
        let full = if artifact.is_absolute() {
            artifact.clone()
        } else {
            driver.meta_root.join(artifact)
        };
        let digest = hash::digest_file(&full).map_err(Stop::Fault)?;
        let mut entry = Map::new();
        entry.insert(
            "path".into(),
            Value::String(relative_to(&full, &driver.meta_root)),
        );
        entry.insert("sha256".into(), Value::String(digest));
        listed.push(Value::Object(entry));
    }
    let mut fields = ledger::null_fields(ledger::Kind::Note);
    fields.insert("text".into(), Value::String(text.to_string()));
    // required and permitted to be EMPTY: a kind whose writer sometimes omits a field has no
    // fixed key set, and the strict reader would then have nothing to decide against.
    fields.insert("artifacts".into(), Value::Array(listed));
    ledger::append(path, ledger::Kind::Note, seq, cycle, fields).map_err(Stop::Fault)?;
    redact::emit(&format!("RECORDED note seq={seq} file={}", path.display()));
    Ok(EXIT_OK)
}

fn relative_to(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// The ledger this invocation writes to, and the three values read off it before any write.
struct Target {
    repo_id: String,
    path: PathBuf,
    records: Vec<ledger::Record>,
    seq: u64,
    cycle: Option<String>,
}

fn verdict(driver: &Driver, args: &RecordArgs, target: &Target, file: &Path) -> Result<i32, Stop> {
    let corpus = args.corpus.ok_or_else(|| refuse("corpus-required"))?;
    let resolved = if file.is_absolute() {
        file.to_path_buf()
    } else {
        driver.meta_root.join(file)
    };
    location(driver, &resolved, corpus, &target.repo_id, args.preflight)?;
    if !resolved.is_file() {
        return Err(Stop::Missing(format!(
            "no verdict file at {}",
            resolved.display()
        )));
    }

    // The digest is over the file's bytes exactly as written, BEFORE any parsing.
    let digest = hash::digest_file(&resolved).map_err(Stop::Fault)?;
    let text =
        std::fs::read_to_string(&resolved).map_err(|err| Stop::Fault(anyhow::Error::new(err)))?;
    let parsed: Value = serde_json::from_str(&text)
        .map_err(|err| refuse(&format!("malformed-json {}: {err}", resolved.display())))?;
    let Value::Object(obj) = parsed else {
        return Err(refuse("not-an-object a verdict file is one JSON object"));
    };
    let checked = validate(driver, &obj, corpus)?;

    // §6.1 routes the first ABORT to a re-spawn and the second to an environment fault.
    if checked.overall == "ABORT" {
        let aborts = target
            .records
            .iter()
            .filter(|record| record.kind == ledger::Kind::Verdict)
            .filter(|record| record.str_field("task") == Some(checked.task.as_str()))
            .filter(|record| record.str_field("overall") == Some("ABORT"))
            .count();
        if aborts >= 2 {
            return Err(refuse("abort-loop a third ABORT for one attempt"));
        }
    }

    // §3.4 condition 3, on the experiment corpus ONLY. The meta, unpackaged and calibration cases
    // record `gh_check: null`: a meta-task or a fenced repair lands in the SUBJECT repository by
    // the orchestrator's push after the gate, not on the experiment repo's `main`.
    let mut gh_ok = true;
    let gh_check = if corpus == Corpus::Experiment && checked.overall == "PASS" {
        let (value, ok) = compare_on_main(driver, &target.repo_id, &checked.pushed_sha);
        gh_ok = ok;
        value
    } else {
        Value::Null
    };

    let mut fields = ledger::null_fields(ledger::Kind::Verdict);
    fields.insert("task".into(), Value::String(checked.task.clone()));
    fields.insert("corpus".into(), Value::String(corpus.as_str().to_string()));
    fields.insert(
        "fixture".into(),
        match &args.fixture {
            Some(name) => Value::String(name.clone()),
            None => Value::Null,
        },
    );
    fields.insert(
        "verdict_file".into(),
        Value::String(relative_to(&resolved, &driver.meta_root)),
    );
    fields.insert("verdict_sha256".into(), Value::String(digest));
    fields.insert("overall".into(), Value::String(checked.overall.clone()));
    fields.insert(
        "verifier".into(),
        Value::String(
            match corpus {
                Corpus::Meta => "meta-verifier",
                _ => "verifier",
            }
            .to_string(),
        ),
    );
    fields.insert(
        "pushed_sha".into(),
        Value::String(checked.pushed_sha.clone()),
    );
    fields.insert("base_sha".into(), Value::String(checked.base_sha.clone()));
    fields.insert("gh_check".into(), gh_check);
    fields.insert("counts".into(), Value::Object(checked.counts.clone()));
    fields.insert(
        "extra_keys".into(),
        Value::Array(
            checked
                .extra_keys
                .iter()
                .map(|key| Value::String(key.clone()))
                .collect(),
        ),
    );
    fields.insert(
        "decisive_evidence".into(),
        obj.get("decisive_evidence").cloned().unwrap_or(Value::Null),
    );
    // `--diagnosis` is the unpackaged arm's flag and is not shipped here (R-001's flag list), so
    // the key is carried `null` — it is a MEMBER of `verdict`'s canonical set either way.
    fields.insert("diagnosis_id".into(), Value::Null);

    // A refusal that leaves no record is a refusal nobody can audit, so the record is appended
    // either way and only the exit code differs.
    ledger::append(
        &target.path,
        ledger::Kind::Verdict,
        target.seq,
        target.cycle.as_deref(),
        fields,
    )
    .map_err(Stop::Fault)?;
    if !gh_ok {
        return Err(refuse(&format!(
            "sha-not-on-main {} is not an ancestor of main",
            checked.pushed_sha
        )));
    }
    redact::emit(&format!(
        "RECORDED verdict seq={} task={} corpus={} overall={}",
        target.seq,
        checked.task,
        corpus.as_str(),
        checked.overall
    ));
    Ok(EXIT_OK)
}

/// The path must lie **directly inside** `<meta_root>/selfhost/state/<repo-id>/verdicts/`, where
/// `<repo-id>` is one the corpus table permits. Parent-equality on the canonicalized leaf — the
/// same predicate shape as the `file://` resolver's, and for the same reason: it refuses
/// redirection to anywhere-that-is-not-base rather than to an enumerated list of bad places.
fn location(
    driver: &Driver,
    resolved: &Path,
    corpus: Corpus,
    repo_id: &str,
    preflight: bool,
) -> Result<(), Stop> {
    let permitted: Vec<String> = match corpus {
        Corpus::Calibration => vec!["calibration".to_string()],
        _ if preflight => vec![PREFLIGHT.to_string()],
        _ => vec![repo_id.to_string()],
    };
    let parent = resolved.parent().map(clean).unwrap_or_default();
    for id in &permitted {
        let expected = clean(&driver.state_dir().join(id).join("verdicts"));
        if parent == expected {
            return Ok(());
        }
    }
    Err(refuse(&format!(
        "outside-verdicts-dir {} is not directly inside {}",
        resolved.display(),
        permitted
            .iter()
            .map(|id| driver
                .state_dir()
                .join(id)
                .join("verdicts")
                .display()
                .to_string())
            .collect::<Vec<_>>()
            .join(" or ")
    )))
}

/// Canonicalize when the path exists; otherwise normalize it lexically, so a comparison never
/// silently succeeds on a path that is not there.
fn clean(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()))
}

/// The fields the ledger record copies out of a validated verdict file.
struct Checked {
    task: String,
    overall: String,
    pushed_sha: String,
    base_sha: String,
    counts: Map<String, Value>,
    extra_keys: Vec<String>,
}

fn is_sha(value: &str) -> bool {
    value.len() == 40
        && value
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
}

fn validate(driver: &Driver, obj: &Map<String, Value>, corpus: Corpus) -> Result<Checked, Stop> {
    if obj.get("schema").and_then(Value::as_str) != Some(VERDICT_SCHEMA) {
        return Err(refuse("schema a verdict file is selfhost.verdict/v1"));
    }
    let task = obj
        .get("task")
        .and_then(Value::as_str)
        .ok_or_else(|| refuse("task-missing"))?
        .to_string();
    arm(driver, &task, corpus)?;
    let overall = obj
        .get("overall")
        .and_then(Value::as_str)
        .ok_or_else(|| refuse("overall-missing"))?
        .to_string();
    if !["PASS", "FAIL", "ABORT"].contains(&overall.as_str()) {
        return Err(refuse(&format!("overall {overall} is not PASS|FAIL|ABORT")));
    }
    let pushed_sha = obj
        .get("pushed_sha")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let base_sha = obj
        .get("base_sha")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if !is_sha(&pushed_sha) || !is_sha(&base_sha) {
        return Err(refuse("sha pushed_sha and base_sha are 40 lowercase hex"));
    }
    if !obj.contains_key("artifact_runs") {
        return Err(refuse("artifact_runs-missing"));
    }
    let decisive = obj
        .get("decisive_evidence")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if overall != "PASS" && decisive.trim().is_empty() {
        return Err(refuse("decisive_evidence-required on a non-PASS verdict"));
    }

    let acceptance = rows(obj, "acceptance", &["PASS", "FAIL", "NOT_RUN"])?;
    if acceptance.values().sum::<u64>() == 0 {
        return Err(refuse("acceptance a verdict names at least one criterion"));
    }
    let requirements = rows(obj, "requirements", &["PASS", "FAIL"])?;
    let decisions = rows(
        obj,
        "decisions",
        &["CONFORMS", "VIOLATES", "NOT_APPLICABLE"],
    )?;
    // A verdict whose summary contradicts its own rows is the exact artifact §5.3's calibration
    // exists to catch.
    if overall == "PASS"
        && (acceptance.get("FAIL").copied().unwrap_or(0) > 0
            || requirements.get("FAIL").copied().unwrap_or(0) > 0
            || decisions.get("VIOLATES").copied().unwrap_or(0) > 0)
    {
        return Err(refuse("inconsistent overall PASS over a failing row"));
    }

    let mut extra_keys: Vec<String> = obj
        .keys()
        .filter(|key| !VERDICT_KEYS.contains(&key.as_str()))
        .cloned()
        .collect();
    extra_keys.sort();
    for key in &extra_keys {
        if let Some(known) = near_miss(key) {
            // A warning that names both spellings finds the typo; a refusal that names none finds
            // the author's next workaround.
            redact::eemit(&format!("SCHEMA-WARN near-miss-key {key} ~ {known}"));
        }
    }

    let mut counts = Map::new();
    counts.insert("acceptance".into(), tally(&acceptance));
    counts.insert("requirements".into(), tally(&requirements));
    counts.insert("decisions".into(), tally(&decisions));
    Ok(Checked {
        task,
        overall,
        pushed_sha,
        base_sha,
        counts,
        extra_keys,
    })
}

fn tally(counts: &BTreeMap<String, u64>) -> Value {
    Value::Object(
        counts
            .iter()
            .map(|(key, value)| (key.clone(), Value::from(*value)))
            .collect(),
    )
}

/// Count one row array's results, refusing an unknown enum member, an empty evidence string and a
/// duplicate id.
fn rows(
    obj: &Map<String, Value>,
    field: &str,
    enums: &[&str],
) -> Result<BTreeMap<String, u64>, Stop> {
    let array = obj
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| refuse(&format!("{field} is an array")))?;
    let mut counts: BTreeMap<String, u64> =
        enums.iter().map(|name| ((*name).to_string(), 0)).collect();
    let mut seen: Vec<&str> = Vec::new();
    for row in array {
        let id = row
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| refuse(&format!("{field} row has no id")))?;
        if seen.contains(&id) {
            return Err(refuse(&format!("{field} id {id} is not unique")));
        }
        seen.push(id);
        let result = row
            .get("result")
            .and_then(Value::as_str)
            .ok_or_else(|| refuse(&format!("{field} row {id} has no result")))?;
        if !enums.contains(&result) {
            return Err(refuse(&format!("{field} row {id} result {result}")));
        }
        if row
            .get("evidence")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            return Err(refuse(&format!("{field} row {id} has no evidence")));
        }
        *counts.entry(result.to_string()).or_default() += 1;
    }
    Ok(counts)
}

/// The schema arm the `task` grammar is decided on (§4.6.6). The unpackaged arm needs
/// `--diagnosis`, which this package does not ship, so the two packaged arms are what is reachable.
fn arm(driver: &Driver, task: &str, corpus: Corpus) -> Result<(), Stop> {
    let numeric = task.len() == 8
        && task.starts_with("TASK-")
        && task[5..].chars().all(|c| c.is_ascii_digit());
    match corpus {
        Corpus::Meta => {
            if !(numeric && task.as_bytes()[5] == b'1') {
                return Err(refuse(&format!("task {task} is not a meta package id")));
            }
            let package = driver.meta_root.join("selfhost").join("tasks").join(task);
            if !package.is_dir() {
                return Err(refuse(&format!(
                    "task {task} has no package at {}",
                    package.display()
                )));
            }
        }
        Corpus::Experiment | Corpus::Calibration => {
            if !numeric {
                return Err(refuse(&format!("task {task} is not a chain task id")));
            }
            let package = driver.subject.tasks_dir().join(task);
            if !package.is_dir() {
                return Err(refuse(&format!(
                    "task {task} has no package at {}",
                    package.display()
                )));
            }
        }
    }
    Ok(())
}

/// Damerau-Levenshtein ≤ 2 against a known key of the same schema (§4.6.2b).
fn near_miss(key: &str) -> Option<&'static str> {
    VERDICT_KEYS
        .into_iter()
        .find(|known| *known != key && distance(key, known) <= 2)
}

fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev2: Vec<usize> = Vec::new();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                cur[j] = cur[j].min(prev2[j - 2] + 1);
            }
        }
        prev2 = std::mem::replace(&mut prev, cur);
    }
    prev[b.len()]
}

/// §3.4 condition 3, in one un-paginated call. Passes iff the status is `identical` or `ahead`,
/// which is exactly "`pushed_sha` is an ancestor of `main`".
fn compare_on_main(driver: &Driver, repo_id: &str, pushed_sha: &str) -> (Value, bool) {
    let owner = &driver.subject.cfg.github.owner;
    let endpoint = format!("repos/{owner}/{repo_id}/compare/{pushed_sha}...main");
    let args = vec![
        "api".to_string(),
        endpoint.clone(),
        "--jq".to_string(),
        ".status".to_string(),
    ];
    let mut cmd = std::process::Command::new("gh");
    cmd.args(&args);
    let captured = ops::capture(&mut cmd).unwrap_or_default();
    let status = captured.stdout.trim().to_string();
    let ok = status == "identical" || status == "ahead";
    let mut check = Map::new();
    check.insert(
        "command".into(),
        Value::String(format!("gh api {endpoint} --jq .status")),
    );
    check.insert("status".into(), Value::String(status));
    check.insert("ok".into(), Value::Bool(ok));
    (Value::Object(check), ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_verdict_key_set_is_the_schemas_own() {
        assert_eq!(VERDICT_KEYS.len(), 10);
        assert!(VERDICT_KEYS.contains(&"artifact_runs"));
        assert!(VERDICT_KEYS.contains(&"decisive_evidence"));
    }

    #[test]
    fn a_forty_hex_sha_is_a_sha_and_nothing_else_is() {
        assert!(is_sha("1056028fc2a846e7ed3b73959328e4f9444b51f8"));
        assert!(!is_sha("1056028F"));
        assert!(!is_sha(&"a".repeat(41)));
        assert!(!is_sha(&"A".repeat(40)));
    }

    #[test]
    fn near_miss_fires_at_distance_two_and_not_beyond() {
        assert_eq!(near_miss("decisive_evidenc"), Some("decisive_evidence"));
        assert_eq!(near_miss("artifact_run"), Some("artifact_runs"));
        assert_eq!(near_miss("clone"), None);
        assert_eq!(near_miss("notes"), None);
    }

    #[test]
    fn distance_counts_a_transposition_as_one() {
        assert_eq!(distance("task", "task"), 0);
        assert_eq!(distance("tsak", "task"), 1);
        assert_eq!(distance("overall", "overal"), 1);
        assert_eq!(distance("abc", "xyz"), 3);
    }

    #[test]
    fn rows_refuse_a_duplicate_id_an_unknown_enum_and_empty_evidence() {
        let ok: Map<String, Value> = serde_json::from_str(
            r#"{"acceptance":[{"id":"AC-001","result":"PASS","command":"true","evidence":"e"}]}"#,
        )
        .unwrap();
        let counts = rows(&ok, "acceptance", &["PASS", "FAIL", "NOT_RUN"]).unwrap();
        assert_eq!(counts.get("PASS"), Some(&1));
        assert_eq!(counts.get("FAIL"), Some(&0));

        for bad in [
            r#"{"acceptance":[{"id":"AC-001","result":"PASS","evidence":"e"},{"id":"AC-001","result":"PASS","evidence":"e"}]}"#,
            r#"{"acceptance":[{"id":"AC-001","result":"MAYBE","evidence":"e"}]}"#,
            r#"{"acceptance":[{"id":"AC-001","result":"PASS","evidence":"  "}]}"#,
            r#"{"acceptance":{}}"#,
        ] {
            let obj: Map<String, Value> = serde_json::from_str(bad).unwrap();
            assert!(
                rows(&obj, "acceptance", &["PASS", "FAIL", "NOT_RUN"]).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_calibration_fixture_name_must_equal_the_basename() {
        let path = Path::new("selfhost/state/calibration/verdicts/some-fixture.json");
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        assert_eq!(stem, "some-fixture");
        assert_ne!(stem, "other-name");
    }
}
