//! `taskfmt selftest` — prove the dispatch tools and the gate on a throwaway fixture. No network,
//! no toolchain: lint accepts the example and rejects broken contracts; progress-init output is
//! verify-shaped; the gate FAILS on the fresh progress file, FAILS on each tamper, and PASSES only
//! on the fully-checked DONE file. Also enforces the repo-wide `CLAUDE.md -> AGENTS.md` rule.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::gate::{self, GateOpts};
use crate::ops::{self, git};
use crate::redact;

/// Directories the symlink audit must never descend into.
const SKIP_DIRS: &[&str] = &[".git", "target", "runs", "node_modules", "fixtures"];

#[derive(Debug, Clone)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub checks: Vec<Check>,
}

impl Report {
    fn push(&mut self, name: &str, ok: bool, detail: Vec<String>) {
        self.checks.push(Check {
            name: name.to_string(),
            ok,
            detail,
        });
    }

    pub fn ok(&self) -> bool {
        self.checks.iter().all(|check| check.ok)
    }

    pub fn failed(&self) -> Vec<&Check> {
        self.checks.iter().filter(|check| !check.ok).collect()
    }

    pub fn render(&self) -> String {
        let mut lines = Vec::new();
        for check in &self.checks {
            if check.ok {
                lines.push(format!("ok   {:<46}", check.name));
            } else {
                lines.push(format!("FAIL {:<46}", check.name));
                for line in &check.detail {
                    lines.push(format!("  | {line}"));
                }
            }
        }
        let verdict = if self.ok() {
            "PASS".to_string()
        } else {
            format!("FAIL ({})", self.failed().len())
        };
        lines.push(format!("SELFTEST {verdict}"));
        lines.join("\n")
    }
}

/// Repo root the crate lives in (`harness/..`).
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

/// Run every scenario. Reads no config and no secrets.
pub fn run() -> Report {
    let mut report = Report::default();
    let root = repo_root();
    let harness = Path::new(env!("CARGO_MANIFEST_DIR"));
    let example = harness.join("testdata/example");
    let template = root.join("reference/task-template");

    // ---- repo rule: every AGENTS.md has a sibling CLAUDE.md symlink pointing at it ----
    let broken = symlink_violations(&root);
    report.push(
        "repo: AGENTS.md/CLAUDE.md symlinks",
        broken.is_empty(),
        broken
            .iter()
            .map(|p| format!("no CLAUDE.md -> AGENTS.md symlink next to {}", p.display()))
            .collect(),
    );

    // ---- repo rule: a relative path carried by two or more task packages under `trusted/` is
    // the same bytes in every one of them, unless it is declared to evolve ----
    let corpus = trusted_findings(&root.join("experiments/tasks"), TRUSTED_EVOLVING);
    report.push(
        "trusted: identity holds",
        corpus.identity.is_empty(),
        corpus.identity,
    );
    report.push(
        "trusted: evolution forward-only",
        corpus.evolution.is_empty(),
        corpus.evolution,
    );

    // ---- the same rule proved in the rejecting direction, on synthetic corpora of invented
    // names: a divergence under a `fixtures` directory, which the walk must descend into; a
    // declared path returning to an earlier content; and a corpus too thin to compare ----
    let synthetic = tempfile::tempdir().expect("tempdir");
    let diverged = synthetic.path().join("diverged");
    plant_corpus(
        &diverged,
        &[
            ("alpha", "held.txt", "held"),
            ("alpha", "nested/fixtures/data.bin", "one"),
            ("beta", "held.txt", "held"),
            ("beta", "nested/fixtures/data.bin", "one"),
            ("gamma", "held.txt", "held"),
            ("gamma", "nested/fixtures/data.bin", "two!!"),
        ],
    );
    let planted = trusted_findings(&diverged, TRUSTED_EVOLVING);
    report.push(
        "trusted: divergence rejected and named",
        planted.identity
            == [
                "nested/fixtures/data.bin len=3 packages=alpha,beta",
                "nested/fixtures/data.bin len=5 packages=gamma",
            ]
            && planted.evolution.is_empty(),
        planted.identity,
    );

    let reverted = synthetic.path().join("reverted");
    plant_corpus(
        &reverted,
        &[
            ("alpha", "moving.txt", "first"),
            ("beta", "moving.txt", "second"),
            ("gamma", "moving.txt", "first"),
        ],
    );
    let reversion = trusted_findings(&reverted, &["moving.txt"]);
    report.push(
        "trusted: reversion rejected",
        reversion.evolution
            == [
                "moving.txt len=5 packages=alpha,gamma",
                "moving.txt len=6 packages=beta",
            ]
            && reversion.identity.is_empty(),
        reversion.evolution,
    );

    let thin = synthetic.path().join("thin");
    std::fs::create_dir_all(thin.join("alpha")).expect("thin package");
    let none = trusted_findings(&thin, TRUSTED_EVOLVING);
    plant_corpus(&thin, &[("alpha", "held.txt", "held")]);
    let one = trusted_findings(&thin, TRUSTED_EVOLVING);
    report.push(
        "trusted: empty corpus rejected",
        none.identity == ["fewer than two packages carry trusted material: packages=0"]
            && one.identity == ["fewer than two packages carry trusted material: packages=1"]
            && none.evolution.is_empty()
            && one.evolution.is_empty(),
        none.identity.into_iter().chain(one.identity).collect(),
    );

    // ---- lint ----
    let example_report = crate::lint::lint_path(&example);
    report.push(
        "lint: example passes",
        example_report.passed(),
        example_report
            .render()
            .lines()
            .map(str::to_string)
            .collect(),
    );
    let template_report = crate::lint::lint_path(&template);
    report.push(
        "lint: template has placeholders",
        !template_report.passed(),
        template_report
            .render()
            .lines()
            .map(str::to_string)
            .collect(),
    );

    let tmp = tempfile::tempdir().expect("tempdir");
    let bad = tmp.path().join("bad");
    std::fs::create_dir_all(&bad).expect("bad dir");
    let readme = std::fs::read_to_string(example.join("README.md")).expect("example README");
    for (name, text) in lint_mutants(&readme) {
        std::fs::write(bad.join("README.md"), &text).expect("write mutant");
        let mutant = crate::lint::lint_path(&bad);
        report.push(
            &format!("lint: {name}"),
            !mutant.passed(),
            mutant.render().lines().map(str::to_string).collect(),
        );
    }

    // ---- workspace + task fixture for the gate matrix ----
    let workspace = tmp.path().join("work");
    let task_dir = tmp.path().join("task");
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::create_dir_all(&task_dir).expect("task dir");
    std::fs::write(task_dir.join("README.md"), &readme).expect("fixture README");
    std::fs::write(task_dir.join("verify.toml"), GATE_VERIFY_TOML).expect("fixture verify.toml");
    if let Err(err) = init_baseline(&workspace) {
        report.push("workspace: baseline commit", false, vec![err.to_string()]);
        return report;
    }

    // ---- progress-init shape ----
    let fresh = tmp.path().join("progress.md");
    if let Err(err) = crate::cmds::progress_init::generate_and_write(&example, Some(&fresh)) {
        report.push("progress-init: generates", false, vec![err.to_string()]);
        return report;
    }
    let progress_text = std::fs::read_to_string(&fresh).unwrap_or_default();
    let fences = progress_text.matches("\n---\n").count()
        + usize::from(progress_text.starts_with("---\n"))
        + usize::from(progress_text.ends_with("\n---"));
    report.push(
        "progress-init: fenced header",
        progress_text.starts_with("---\n") && fences >= 2,
        vec![
            progress_text
                .lines()
                .take(2)
                .collect::<Vec<_>>()
                .join(" | "),
        ],
    );
    let header_shape = [
        "TASK: TASK-042",
        "STATE: IN_PROGRESS",
        "CURRENT: 1.1",
        "BASELINE: <not run>",
    ]
    .iter()
    .all(|want| progress_text.lines().any(|line| line.starts_with(want)));
    report.push("progress-init: header shape", header_shape, vec![]);
    let source = taskfile_checklist(&readme);
    let generated = taskfile_checklist(&progress_text);
    report.push(
        "progress-init: checklist verbatim",
        source == generated,
        vec![format!(
            "source {} lines vs generated {} lines",
            source.len(),
            generated.len()
        )],
    );

    // ---- gate matrix ----
    let logs = tmp.path().join("logs");
    let gate_at = |progress: &Path| -> crate::gate::GateOutput {
        gate::run(GateOpts {
            root: workspace.clone(),
            task_dir: task_dir.clone(),
            progress: Some(progress.display().to_string()),
            base: Some("baseline".to_string()),
            log_dir: Some(logs.clone()),
            fail_fast: false,
            enforce_task_contract: false,
        })
    };

    let fresh_gate = gate_at(&fresh);
    report.push(
        "gate: fresh progress fails",
        !fresh_gate.is_pass(),
        vec![format!(
            "exit {} last line {:?}",
            fresh_gate.exit, fresh_gate.last_line
        )],
    );

    let done_path = tmp.path().join("done.md");
    let done = to_done(&progress_text);
    std::fs::write(&done_path, &done).expect("done progress");
    let done_gate = gate_at(&done_path);
    report.push(
        "gate: done progress passes DONE",
        done_gate.is_pass(),
        vec![format!(
            "exit {} last line {:?}",
            done_gate.exit, done_gate.last_line
        )]
        .into_iter()
        .chain(
            done_gate
                .text
                .lines()
                .filter(|l| l.starts_with("CHECK"))
                .map(str::to_string),
        )
        .collect(),
    );

    for (name, text) in tamper_matrix(&done) {
        let path = tmp
            .path()
            .join(format!("{}.md", name.replace([':', ' '], "-")));
        std::fs::write(&path, &text).expect("tamper file");
        if std::env::var("TASKFMT_SELFTEST_DEBUG").as_deref() == Ok("1") {
            std::fs::write(
                format!("/tmp/st-debug/{}.md", name.replace([':', ' ', ','], "-")),
                &text,
            )
            .expect("debug tamper");
        }
        let output = gate_at(&path);
        report.push(
            &format!("gate: {name}"),
            !output.is_pass(),
            vec![format!(
                "exit {} last line {:?}",
                output.exit, output.last_line
            )],
        );
    }

    let missing = gate_at(&tmp.path().join("nope.md"));
    report.push(
        "gate: missing progress file",
        !missing.is_pass(),
        vec![format!(
            "exit {} last line {:?}",
            missing.exit, missing.last_line
        )],
    );

    // ---- command lists run under `bash -eo pipefail`: an early failing statement and a failing
    // pipe stage both fail the check; a clean pipe passes ----
    let task2 = tmp.path().join("task2");
    std::fs::create_dir_all(&task2).expect("task2 dir");
    std::fs::write(task2.join("README.md"), &readme).expect("task2 README");
    for (name, command, want_pass) in COMMAND_CASES {
        let toml = GATE_VERIFY_TOML.replacen(
            "[focused]\ncommands = []",
            &format!("[focused]\ncommands = [{command:?}]"),
            1,
        );
        std::fs::write(task2.join("verify.toml"), toml).expect("task2 verify.toml");
        let output = gate::run(GateOpts {
            root: workspace.clone(),
            task_dir: task2.clone(),
            progress: Some(done_path.display().to_string()),
            base: Some("baseline".to_string()),
            log_dir: Some(logs.clone()),
            fail_fast: false,
            enforce_task_contract: false,
        });
        let ok = if *want_pass {
            output.is_pass()
        } else {
            !output.is_pass() && output.failed_checks == ["focused.1"]
        };
        report.push(
            &format!("cmds: {name}"),
            ok,
            vec![format!(
                "exit {} last line {:?} failed {:?}",
                output.exit, output.last_line, output.failed_checks
            )],
        );
    }

    report
}

/// (name, focused command, whole-gate verdict) — the git-free command-semantics cases mirrored
/// from `tests/gate_tamper_matrix.rs`, so `taskfmt selftest` proves them with no toolchain and no
/// network. Not inside a container image, though, and that claim used to stand here: an image
/// ships the binary alone, `repo_root()` is the build-time `CARGO_MANIFEST_DIR`, and `run()`
/// panics on the absent example package long before these cases. Measured — `docker run --rm
/// harness-taskfmt:latest selftest` panics reading the example README. Nothing invokes `selftest`
/// in a container, so nothing depends on it.
const COMMAND_CASES: &[(&str, &str, bool)] = &[
    (
        "'false; true' fails focused.1 (errexit)",
        "false; true",
        false,
    ),
    ("'true | true' passes", "true | true", true),
    (
        "'false | true' fails focused.1 (pipefail)",
        "false | true",
        false,
    ),
];

/// The broken-contract mutants selftest must reject.
fn lint_mutants(readme: &str) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    // breaks the line grammar: the ID token is no longer `N.N`
    let broken = readme
        .lines()
        .map(|line| {
            if line.contains("**3.2**") {
                line.replace("**3.2**", "**3.2X**")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    out.push(("broken checklist grammar", broken));
    // 3.2 without 3.1 => non-contiguous
    let noncontig = readme
        .lines()
        .filter(|l| !l.contains("**3.1**"))
        .collect::<Vec<_>>()
        .join("\n");
    out.push(("non-contiguous IDs", noncontig));
    // H1 no longer matches the id
    let mismatch = readme.replacen("id: TASK-042", "id: TASK-043", 1);
    out.push(("H1/id mismatch", mismatch));
    out.push((
        "empty leaf evidence (C1)",
        readme.replacen(
            "evidence: `cargo test -p auth valid_refresh_rotation` → `1 passed`.",
            "evidence: .",
            1,
        ),
    ));
    out.push((
        "duplicate R definition (C2)",
        readme.replacen("- **R-003 (MUST NOT):**", "- **R-002 (MUST NOT):**", 1),
    ));
    out.push((
        "uncited requirement (C3)",
        readme
            .replacen("- **Covers:** `R-001, R-003`", "- **Covers:** `R-001`", 1)
            .replacen("(`R-001`, `R-003`)", "(`R-001`)", 1),
    ));
    out.push((
        "template placeholder left (C4)",
        readme.replacen(
            "Expired refresh tokens are rejected before any session state is rotated.",
            "Given <state>, tokens are rejected.",
            1,
        ),
    ));
    out.push((
        "AC cited only on a bare parent (C5)",
        readme
            .replacen(
                "(`AC-002`) — evidence: `cargo test -p auth valid_refresh_rotation` → `1 passed`.",
                "— evidence: `cargo test -p auth valid_refresh_rotation` exits 0.",
                1,
            )
            .replacen(
                "- [ ] **2** Required behavior is implemented.",
                "- [ ] **2** Required behavior is implemented (`AC-002`).",
                1,
            ),
    ));
    out.push((
        "identical evidence on two items (C5)",
        readme.replacen(
            "evidence: `cargo test -p auth valid_refresh_rotation` → `1 passed`.",
            "evidence: `cargo test -p auth expired_refresh_token` exits 0.",
            1,
        ),
    ));
    out.push((
        "hint references /task/ (C8)",
        readme.replacen(
            "4. `docs/decisions/D-041.md` — error-code contract.",
            "4. `/task/decisions.md` — error-code contract.",
            1,
        ),
    ));
    out
}

/// Every tamper must leave the gate red: the progress file is the only thing the agent may edit.
fn tamper_matrix(done: &str) -> Vec<(&'static str, String)> {
    vec![
        (
            "STATE not DONE",
            done.replacen("STATE: DONE", "STATE: IN_PROGRESS", 1),
        ),
        (
            "CURRENT not NONE",
            done.replacen("CURRENT: NONE", "CURRENT: 3.2", 1),
        ),
        (
            "BASELINE not recorded",
            replace_line_starting(done, "BASELINE: ", "BASELINE: <not run>"),
        ),
        (
            "TASK id mismatch",
            done.replacen("TASK: TASK-042", "TASK: TASK-041", 1),
        ),
        (
            "parent checked, child not",
            replace_checkbox(done, "    - [x] **3.1**", "    - [ ] **3.1**"),
        ),
        (
            "parent unchecked, kids done",
            replace_checkbox(done, "- [x] **3**", "- [ ] **3**"),
        ),
        ("reworded checklist text", reword(done, "**3.1**")),
        (
            "deleted checklist line",
            drop_line_containing(done, "**3.2**"),
        ),
        ("added checklist line", append_after_checklist(done)),
    ]
}

fn replace_line_starting(text: &str, prefix: &str, replacement: &str) -> String {
    text.lines()
        .map(|line| {
            if line.starts_with(prefix) {
                replacement.to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn replace_checkbox(text: &str, from: &str, _to: &str) -> String {
    text.lines()
        .map(|line| {
            if line.trim_start().starts_with(from.trim()) {
                line.replacen("- [x]", "- [ ]", 1)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn reword(text: &str, marker: &str) -> String {
    text.lines()
        .map(|line| {
            if line.contains(marker) {
                line.replacen(marker, &format!("{marker} (reworded)"), 1)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn drop_line_containing(text: &str, marker: &str) -> String {
    format!(
        "{}\n",
        text.lines()
            .filter(|line| !line.contains(marker))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn append_after_checklist(text: &str) -> String {
    // inside the markers: an added row outside them would not be part of the compared block
    let mut out = Vec::new();
    for line in text.lines() {
        if line.trim() == crate::taskfile::CHECKLIST_END.trim() {
            out.push("    - [x] **3.3** extra — evidence: none.".to_string());
        }
        out.push(line.to_string());
    }
    out.join("\n") + "\n"
}

/// `git init`, one empty commit, tag `baseline` — the gate's scope check needs it.
fn init_baseline(workspace: &Path) -> anyhow::Result<()> {
    let mut init = std::process::Command::new("git");
    init.current_dir(workspace).args(["init", "-q"]);
    let captured = ops::capture(&mut init)?;
    if !captured.ok() {
        anyhow::bail!("git init failed: {}", captured.stderr.trim());
    }
    git::commit(workspace, "baseline", false, true)?;
    git::tag(workspace, "baseline")
}

fn taskfile_checklist(text: &str) -> Vec<String> {
    crate::taskfile::checklist_block(text)
}

/// The done-state transform from selftest.sh: every box checked, header complete.
fn to_done(progress: &str) -> String {
    let done = progress.replace("- [ ]", "- [x]");
    replace_line_starting(
        &replace_line_starting(
            &replace_line_starting(&done, "STATE: ", "STATE: DONE"),
            "CURRENT: ",
            "CURRENT: NONE",
        ),
        "BASELINE: ",
        "BASELINE: cargo test -p auth expired_refresh_token -> 1 failed",
    )
}

/// Every AGENTS.md in `root` (minus `SKIP_DIRS`) must have a sibling `CLAUDE.md` symlink.
pub fn symlink_violations(root: &Path) -> Vec<PathBuf> {
    let mut violations = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            entry.file_name().to_string_lossy().as_ref() != ".git"
                && !SKIP_DIRS.contains(&entry.file_name().to_string_lossy().as_ref())
        })
    {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() || entry.file_name() != "AGENTS.md" {
            continue;
        }
        let dir = entry
            .path()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let link = dir.join("CLAUDE.md");
        let ok = std::fs::symlink_metadata(&link)
            .map(|meta| meta.file_type().is_symlink())
            .unwrap_or(false)
            && std::fs::read_link(&link)
                .map(|target| target == Path::new("AGENTS.md"))
                .unwrap_or(false);
        if !ok {
            violations.push(entry.path().to_path_buf());
        }
    }
    violations.sort();
    violations
}

/// Relative paths under `trusted/` declared to differ between task packages because each copy
/// compiles against the API surface its own package asks an implementer to build. Every other
/// path carried by two or more packages must be byte-identical. An entry here drops a path out of
/// that enforcement for good, so this constant is the single, reviewable way to widen the
/// exception: the file is fenced, and adding a line costs an adversarial debate round and a
/// minted repair token.
const TRUSTED_EVOLVING: &[&str] = &["crates/pgtui/tests/support/mod.rs"];

/// One package's copy of a shared relative path: the package directory name and its bytes.
type Carrier = (String, Vec<u8>);

/// The `trusted/` trees of one corpus, keyed by relative path.
struct TrustedCorpus {
    /// Packages carrying a `trusted/` directory at all — fewer than two can compare nothing.
    carriers: usize,
    /// Relative path -> its copies, in ascending package order.
    copies: BTreeMap<String, Vec<Carrier>>,
}

/// Verdict on one corpus. Both vectors hold ready-to-print detail lines and are empty when the
/// invariant holds; a path that does not offend is never named.
#[derive(Default)]
struct TrustedFindings {
    identity: Vec<String>,
    evolution: Vec<String>,
}

/// Read `<corpus>/<package>/trusted/**`. Only direct children of `corpus` are packages, so no
/// nested copy of a corpus elsewhere in the tree can join the comparison. The walk descends into
/// every directory: `SKIP_DIRS` belongs to the symlink audit and excludes `fixtures`, which the
/// packages share. Bytes are compared directly rather than digested — some trusted files are
/// binary, and no collision argument is then needed.
fn trusted_corpus(corpus: &Path) -> TrustedCorpus {
    let mut packages: Vec<(String, PathBuf)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(corpus) {
        for entry in entries.flatten() {
            let trusted = entry.path().join("trusted");
            if trusted.is_dir() {
                packages.push((entry.file_name().to_string_lossy().into_owned(), trusted));
            }
        }
    }
    packages.sort();
    let mut copies: BTreeMap<String, Vec<Carrier>> = BTreeMap::new();
    for (package, trusted) in &packages {
        for entry in walkdir::WalkDir::new(trusted).follow_links(false) {
            let entry = entry.expect("walk trusted tree");
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(trusted)
                .expect("path under trusted")
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            let bytes = std::fs::read(entry.path()).expect("read trusted file");
            copies
                .entry(rel)
                .or_default()
                .push((package.clone(), bytes));
        }
    }
    TrustedCorpus {
        carriers: packages.len(),
        copies,
    }
}

/// The failure grammar: one line per distinct content of `rel`, `PATH len=N packages=NAMES`, the
/// carriers of a content ascending and the lines sorted, so a reader attributes a content to its
/// packages from a single line and the output is stable.
fn content_groups(rel: &str, copies: &[Carrier]) -> Vec<String> {
    let mut groups: Vec<(&[u8], Vec<&str>)> = Vec::new();
    for (package, bytes) in copies {
        let content = bytes.as_slice();
        match groups.iter().position(|(seen, _)| *seen == content) {
            Some(index) => groups[index].1.push(package),
            None => groups.push((content, vec![package])),
        }
    }
    let mut lines: Vec<String> = groups
        .iter()
        .map(|(content, names)| {
            let mut names = names.clone();
            names.sort_unstable();
            format!("{rel} len={} packages={}", content.len(), names.join(","))
        })
        .collect();
    lines.sort();
    lines
}

/// A declared path is exempt from identity, not from all structure: its contents in package order
/// must form contiguous runs. A content that reappears after a different one intervened is a
/// reversion — and, more to the point, is what an amendment applied to some packages but not
/// others looks like.
fn reverts(copies: &[Carrier]) -> bool {
    let mut seen: Vec<&[u8]> = Vec::new();
    for (index, (_, bytes)) in copies.iter().enumerate() {
        let content = bytes.as_slice();
        if index > 0 && content == copies[index - 1].1.as_slice() {
            continue;
        }
        if seen.contains(&content) {
            return true;
        }
        seen.push(content);
    }
    false
}

/// Judge one corpus against `evolving`, the declared-divergence set. A corpus with fewer than two
/// carriers is reported with the count it does have rather than passing vacuously.
fn trusted_findings(corpus: &Path, evolving: &[&str]) -> TrustedFindings {
    let TrustedCorpus { carriers, copies } = trusted_corpus(corpus);
    let mut findings = TrustedFindings::default();
    if carriers < 2 {
        findings.identity.push(format!(
            "fewer than two packages carry trusted material: packages={carriers}"
        ));
        return findings;
    }
    for (rel, carried) in &copies {
        if evolving.contains(&rel.as_str()) {
            if reverts(carried) {
                findings.evolution.extend(content_groups(rel, carried));
            }
        } else if carried.len() >= 2 {
            let groups = content_groups(rel, carried);
            if groups.len() > 1 {
                findings.identity.extend(groups);
            }
        }
    }
    findings
}

/// Write a synthetic corpus from `(package, relative path, contents)` triples under `dir`.
fn plant_corpus(dir: &Path, files: &[(&str, &str, &str)]) {
    for (package, rel, body) in files {
        let path = dir.join(package).join("trusted").join(rel);
        std::fs::create_dir_all(path.parent().expect("corpus parent")).expect("corpus dir");
        std::fs::write(&path, body).expect("corpus file");
    }
}

/// Minimal verify.toml for the gate matrix: no commands, everything allowed.
const GATE_VERIFY_TOML: &str = r#"schema = "verify/v1"
base_ref = "baseline"
allowed_globs = ["*"]

[focused]
commands = []

[regression]
commands = []

[lint]
commands = []
"#;

/// Console entry point (`taskfmt selftest`): run everything, print, exit 1 on failure.
pub fn console() -> anyhow::Result<i32> {
    let report = run();
    redact::emit_lines(report.render().lines());
    Ok(usize::from(!report.ok()) as i32)
}
