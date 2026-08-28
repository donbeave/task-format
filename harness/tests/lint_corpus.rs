//! Lint corpus: the v4 example must pass, broken variants must not, the template must not.
//! Every rule C1–C9 has one negative case built by mutating the example text, plus the positive
//! twins (an AC cited on an evidence-bearing parent; `<...>` inside a code span).

use std::path::{Path, PathBuf};

use taskfmt::lint::{self, Finding, Severity};
use taskfmt::taskfile::TaskFile;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn example() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/example")
}

fn lint_text(text: &str, readme: &Path) -> Vec<Finding> {
    lint::lint_text(text, readme)
}

fn example_text() -> (String, PathBuf) {
    let readme = example().join("README.md");
    let text = std::fs::read_to_string(&readme).unwrap();
    (text, readme)
}

fn errors(findings: &[Finding]) -> Vec<&Finding> {
    findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .collect()
}

fn has(findings: &[Finding], severity: Severity, rule: &str, needle: &str) -> bool {
    findings
        .iter()
        .any(|f| f.severity == severity && f.rule == rule && f.message.contains(needle))
}

/// Replace exactly one occurrence; a mutation that does not apply is a broken test, not a pass.
fn mutate(text: &str, from: &str, to: &str) -> String {
    assert_eq!(text.matches(from).count(), 1, "mutation anchor: {from:?}");
    text.replacen(from, to, 1)
}

#[test]
fn example_package_passes_clean() {
    let report = lint::lint_path(&example());
    assert!(
        report.passed(),
        "the v4 corpus must lint clean:\n{}",
        report.render()
    );
    assert_eq!(report.errors(), 0);
    // the one intended warning: AC-003 is a `! grep` the gate covers via forbidden_patterns
    let warns: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Warn)
        .collect();
    assert_eq!(warns.len(), 1, "{}", report.render());
    assert!(
        has(
            &report.findings,
            Severity::Warn,
            "acceptance",
            "AC-003 evidence command does not appear verbatim in verify.toml"
        ),
        "{}",
        report.render()
    );
}

#[test]
fn template_still_has_placeholders() {
    let report = lint::lint_path(&repo_root().join("reference/task-template"));
    assert!(
        !report.passed(),
        "the authoring template is not a task package"
    );
    // placeholder findings only (README `<...>` set, hand-listed IDs, verify.toml): the template
    // is otherwise a valid package shape
    assert!(
        report
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .all(|f| f.rule == "placeholders"
                || (f.rule == "config" && f.message.starts_with("template placeholders left"))),
        "{}",
        report.render()
    );
}

#[test]
fn frontmatter_contract() {
    let (text, readme) = example_text();
    let findings = lint_text(&text, &readme);
    assert!(errors(&findings).is_empty(), "{findings:?}");

    // wrong schema
    let bad = text.replacen("schema: task/v4", "schema: task/v3", 1);
    let findings = lint_text(&bad, &readme);
    assert!(
        has(&findings, Severity::Error, "frontmatter", "task/v4"),
        "{findings:?}"
    );

    // id/H1 mismatch
    let bad = text.replacen("id: TASK-042", "id: TASK-099", 1);
    let findings = lint_text(&bad, &readme);
    assert!(findings.iter().any(|f| f.rule == "heading"), "{findings:?}");

    // missing verify command
    let bad = text.replacen("verify: \"taskfmt verify\"", "verify: \"\"", 1);
    let findings = lint_text(&bad, &readme);
    assert!(
        findings
            .iter()
            .any(|f| f.rule == "config" || f.rule == "frontmatter"),
        "{findings:?}"
    );
}

#[test]
fn checklist_contract() {
    let (text, readme) = example_text();

    // a leaf without evidence
    let bad = text
        .lines()
        .map(|line| {
            if line.contains("**3.1**") {
                line.split(" — evidence:").next().unwrap().to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let findings = lint_text(&bad, &readme);
    assert!(
        has(&findings, Severity::Error, "checklist", "evidence"),
        "{findings:?}"
    );

    // a non-contiguous id: 2.3 without 2.2
    let bad = text
        .lines()
        .filter(|l| !l.contains("**2.2**"))
        .collect::<Vec<_>>()
        .join("\n");
    let findings = lint_text(&bad, &readme);
    assert!(
        has(&findings, Severity::Error, "checklist", "expected ID 2.2"),
        "{findings:?}"
    );

    // a broken id token
    let bad = text.replace("**3.2**", "**3.2X**");
    let findings = lint_text(&bad, &readme);
    assert!(
        has(&findings, Severity::Error, "checklist", "bad line"),
        "{findings:?}"
    );

    // depth 5 is rejected
    let bad = text.replace(
        "        - [ ] **2.1.1**",
        "            - [ ] **2.1.1.1.1** deep",
    );
    let findings = lint_text(&bad, &readme);
    assert!(
        findings.iter().any(|f| f.rule == "checklist"),
        "{findings:?}"
    );
}

// ---------- C1: leaf evidence text ----------
#[test]
fn c1_leaf_evidence_needs_a_command_or_exit_claim() {
    let (text, readme) = example_text();
    let anchor = "evidence: `cargo test -p auth valid_refresh_rotation` → `1 passed`.";

    let empty = mutate(&text, anchor, "evidence: .");
    let findings = lint_text(&empty, &readme);
    assert!(
        has(
            &findings,
            Severity::Error,
            "checklist",
            "leaf 2.4 evidence text is empty"
        ),
        "{findings:?}"
    );

    let vague = mutate(&text, anchor, "evidence: the valid path still works.");
    let findings = lint_text(&vague, &readme);
    assert!(
        has(
            &findings,
            Severity::Error,
            "checklist",
            "leaf 2.4 evidence names no backticked command and no 'exit 0' claim"
        ),
        "{findings:?}"
    );

    // an "exits 0" claim without a command is enough
    let claim = mutate(&text, anchor, "evidence: the valid-path test exits 0.");
    let findings = lint_text(&claim, &readme);
    assert!(errors(&findings).is_empty(), "{findings:?}");

    // the gate leaf is exempt from the command/exit claim
    let gate = mutate(
        &text,
        "evidence: final full run (with progress check); full output in the transcript.",
        "evidence: full output in the transcript.",
    );
    let findings = lint_text(&gate, &readme);
    assert!(errors(&findings).is_empty(), "{findings:?}");
}

// ---------- C2: duplicate definitions ----------
#[test]
fn c2_duplicate_definitions_and_ac_rows() {
    let (text, readme) = example_text();

    let dup_r = mutate(&text, "- **R-003 (MUST NOT):**", "- **R-002 (MUST NOT):**");
    let findings = lint_text(&dup_r, &readme);
    assert!(
        has(
            &findings,
            Severity::Error,
            "ids",
            "duplicate definition of R-002"
        ),
        "{findings:?}"
    );

    let dup_d = mutate(&text, "- **D-003:**", "- **D-001:**");
    let findings = lint_text(&dup_d, &readme);
    assert!(
        has(
            &findings,
            Severity::Error,
            "ids",
            "duplicate definition of D-001"
        ),
        "{findings:?}"
    );

    let dup_p = mutate(&text, "- **P-002:**", "- **P-001:**");
    let findings = lint_text(&dup_p, &readme);
    assert!(
        has(
            &findings,
            Severity::Error,
            "ids",
            "duplicate definition of P-001"
        ),
        "{findings:?}"
    );

    let row = text
        .lines()
        .find(|line| line.starts_with("| AC-002 |"))
        .unwrap()
        .to_string();
    let dup_ac = mutate(&text, &row, &format!("{row}\n{row}"));
    let findings = lint_text(&dup_ac, &readme);
    assert!(
        has(&findings, Severity::Error, "ids", "duplicate AC row AC-002"),
        "{findings:?}"
    );
}

// ---------- C3: every R-* cited ----------
#[test]
fn c3_every_requirement_is_cited() {
    let (text, readme) = example_text();

    // R-003 is cited only on 2.1
    let uncited = mutate(&text, "(`R-001`, `R-003`, `AC-001`)", "(`R-001`, `AC-001`)");
    let findings = lint_text(&uncited, &readme);
    assert!(
        has(
            &findings,
            Severity::Error,
            "requirements",
            "R-003 is not cited by any AC row or checklist leaf"
        ),
        "{findings:?}"
    );

    // a citation on the parent covers its leaves, and ranges expand
    let on_parent = mutate(
        &uncited,
        "- [ ] **2** Required behavior is implemented.",
        "- [ ] **2** Required behavior is implemented (`R-001..R-004`).",
    );
    let findings = lint_text(&on_parent, &readme);
    assert!(
        !findings.iter().any(|f| f.rule == "requirements"),
        "{findings:?}"
    );

    // a section with no R-* at all
    let none = text
        .lines()
        .filter(|line| !line.starts_with("- **R-"))
        .collect::<Vec<_>>()
        .join("\n");
    let findings = lint_text(&none, &readme);
    assert!(
        has(
            &findings,
            Severity::Error,
            "requirements",
            "no R-NNN entries"
        ),
        "{findings:?}"
    );
}

// ---------- C4: placeholders derived from the template ----------
#[test]
fn c4_placeholders_derived_from_the_template() {
    let (text, readme) = example_text();
    let goal = "Expired refresh tokens are rejected before any session state is rotated.";

    // a bare template span and a whole-code-span template span both count
    for injected in [
        "Given <state>, tokens are rejected.",
        "Run `<command>` first.",
    ] {
        let bad = mutate(&text, goal, injected);
        let findings = lint_text(&bad, &readme);
        assert!(
            has(
                &findings,
                Severity::Error,
                "placeholders",
                "template <...> placeholders left (set derived from"
            ),
            "{injected}: {findings:?}"
        );
    }

    // `<...>` inside a longer code span is literal: CLI usage, generics
    let literal = mutate(
        &text,
        goal,
        "Run `pgtui --db <path>` and return `Vec<TableRef>`; then tokens are rejected.",
    );
    let findings = lint_text(&literal, &readme);
    assert!(errors(&findings).is_empty(), "{findings:?}");
    assert!(
        !findings.iter().any(|f| f.rule == "placeholders"),
        "{findings:?}"
    );

    // a bare span with a space that is not in the template set is a warning only
    let unfilled = mutate(&text, goal, "Tokens from <the new store> are rejected.");
    let findings = lint_text(&unfilled, &readme);
    assert!(errors(&findings).is_empty(), "{findings:?}");
    assert!(
        has(
            &findings,
            Severity::Warn,
            "placeholders",
            "bare <...> spans outside inline code look unfilled"
        ),
        "{findings:?}"
    );

    // the hand-listed IDs stay errors
    let stale_id = mutate(&text, "- **P-002:**", "- **P-NNN:**");
    let findings = lint_text(&stale_id, &readme);
    assert!(
        has(
            &findings,
            Severity::Error,
            "placeholders",
            "template placeholders left:"
        ),
        "{findings:?}"
    );
}

// ---------- C5: AC coverage and identical evidence ----------
#[test]
fn c5_ac_cited_on_leaf_or_evidence_bearing_parent() {
    let (text, readme) = example_text();
    let leaf_24 = "    - [ ] **2.4** Valid refresh path unchanged (`AC-002`) — evidence: `cargo test -p auth valid_refresh_rotation` → `1 passed`.";
    let parent_2 = "- [ ] **2** Required behavior is implemented.";

    // positive twin: the citation moves to a parent that carries its own evidence
    let on_parent = mutate(
        &mutate(
            &text,
            leaf_24,
            "    - [ ] **2.4** Valid refresh path unchanged — evidence: `cargo test -p auth valid_refresh_rotation` exits 0.",
        ),
        parent_2,
        "- [ ] **2** Required behavior is implemented (`AC-002`) — evidence: `cargo test -p auth valid_refresh_rotation` → `1 passed`.",
    );
    let findings = lint_text(&on_parent, &readme);
    assert!(errors(&findings).is_empty(), "{findings:?}");

    // negative: the parent cites the AC but has no evidence of its own
    let on_bare_parent = mutate(
        &mutate(
            &text,
            leaf_24,
            "    - [ ] **2.4** Valid refresh path unchanged — evidence: `cargo test -p auth valid_refresh_rotation` exits 0.",
        ),
        parent_2,
        "- [ ] **2** Required behavior is implemented (`AC-002`).",
    );
    let findings = lint_text(&on_bare_parent, &readme);
    assert!(
        has(
            &findings,
            Severity::Error,
            "checklist",
            "AC-002 is not cited by any leaf or evidence-bearing parent"
        ),
        "{findings:?}"
    );

    // identical full evidence text on two items (2.1 is a parent, hence "items")
    let twin = mutate(
        &text,
        leaf_24,
        "    - [ ] **2.4** Valid refresh path unchanged (`AC-002`) — evidence: `cargo test -p auth expired_refresh_token` exits 0.",
    );
    let findings = lint_text(&twin, &readme);
    assert!(
        has(
            &findings,
            Severity::Error,
            "checklist",
            "items 2.1 and 2.4 carry identical evidence: `cargo test -p auth expired_refresh_token` exits 0"
        ),
        "{findings:?}"
    );
}

// ---------- C6: AC command run by the gate ----------
#[test]
fn c6_ac_command_absent_from_verify_toml_warns() {
    let (text, readme) = example_text();
    let findings = lint_text(&text, &readme);
    // the corpus carries the intended one: AC-003's `! grep` is covered by a forbidden pattern
    assert!(
        has(
            &findings,
            Severity::Warn,
            "acceptance",
            "AC-003 evidence command does not appear verbatim in verify.toml: ! grep -rn legacy_expiry_check src tests"
        ),
        "{findings:?}"
    );
    assert!(
        !has(
            &findings,
            Severity::Warn,
            "acceptance",
            "AC-001 evidence command"
        ),
        "{findings:?}"
    );

    let renamed = text.replace(
        "cargo test -p auth valid_refresh_rotation",
        "cargo test -p auth valid_rotation_path",
    );
    let findings = lint_text(&renamed, &readme);
    assert!(
        has(
            &findings,
            Severity::Warn,
            "acceptance",
            "AC-002 evidence command does not appear verbatim in verify.toml: cargo test -p auth valid_rotation_path"
        ),
        "{findings:?}"
    );
    assert!(errors(&findings).is_empty(), "{findings:?}");
}

// ---------- C7: cargo test multi-filter ----------
#[test]
fn c7_cargo_test_multi_filter_warns() {
    let (text, readme) = example_text();
    let multi = text.replace(
        "cargo test -p auth valid_refresh_rotation",
        "cargo test -p auth valid_refresh rotation",
    );
    let findings = lint_text(&multi, &readme);
    assert!(
        has(
            &findings,
            Severity::Warn,
            "commands",
            "AC-002: cargo test takes one positional filter; several words without ' -- ' is invalid: cargo test -p auth valid_refresh rotation"
        ),
        "{findings:?}"
    );
    assert!(
        has(
            &findings,
            Severity::Warn,
            "commands",
            "leaf 2.4: cargo test takes one positional filter"
        ),
        "{findings:?}"
    );
    assert!(errors(&findings).is_empty(), "{findings:?}");

    // the ` -- ` separator makes it valid
    let separated = text.replace(
        "cargo test -p auth valid_refresh_rotation",
        "cargo test -p auth -- valid_refresh rotation",
    );
    let findings = lint_text(&separated, &readme);
    assert!(
        !findings.iter().any(|f| f.rule == "commands"),
        "{findings:?}"
    );
}

// ---------- C8: hints never reference /task/ ----------
#[test]
fn c8_read_before_editing_must_not_reference_task_dir() {
    let (text, readme) = example_text();
    let bad = mutate(
        &text,
        "4. `docs/decisions/D-041.md` — error-code contract.",
        "4. `/task/decisions.md` — error-code contract.",
    );
    let findings = lint_text(&bad, &readme);
    assert!(
        has(
            &findings,
            Severity::Error,
            "context",
            "\"Read before editing\" (non-normative hints) must not reference /task/ binding docs:"
        ),
        "{findings:?}"
    );
    assert!(
        has(
            &findings,
            Severity::Error,
            "context",
            "`/task/decisions.md`"
        ),
        "{findings:?}"
    );

    // prose outside the numbered list is not a hint
    let prose = mutate(
        &text,
        "## Goal\n",
        "## Goal\n\nSee `/task/AGENTS.md` for the protocol.\n",
    );
    let findings = lint_text(&prose, &readme);
    assert!(
        !findings.iter().any(|f| f.rule == "context"),
        "{findings:?}"
    );
}

// ---------- C9: baseline command equals an AC command ----------
#[test]
fn c9_baseline_command_must_match_an_ac_command() {
    let (text, readme) = example_text();
    let findings = lint_text(&text, &readme);
    assert!(
        !findings.iter().any(|f| f.rule == "baseline"),
        "{findings:?}"
    );

    let other = mutate(
        &text,
        "```sh\ncargo test -p auth expired_refresh_token\n```",
        "```sh\ncargo test -p auth\n```",
    );
    let findings = lint_text(&other, &readme);
    assert!(
        has(
            &findings,
            Severity::Warn,
            "baseline",
            "Baseline command is not identical to any AC evidence command: cargo test -p auth"
        ),
        "{findings:?}"
    );

    let none = mutate(
        &text,
        "```sh\ncargo test -p auth expired_refresh_token\n```\n",
        "",
    );
    let findings = lint_text(&none, &readme);
    assert!(
        has(
            &findings,
            Severity::Warn,
            "baseline",
            "no fenced command found after \"Baseline\""
        ),
        "{findings:?}"
    );
}

#[test]
fn size_warns_but_does_not_error() {
    let (text, readme) = example_text();
    let long_text = format!(
        "{text}\n{}",
        "filler line to exceed the size budget\n".repeat(400)
    );
    let findings = lint_text(&long_text, &readme);
    assert!(
        has(&findings, Severity::Warn, "size", "bytes"),
        "{findings:?}"
    );
    assert!(
        !findings
            .iter()
            .any(|f| f.severity == Severity::Error && f.rule == "size")
    );
}

#[test]
fn verify_toml_must_sit_next_to_the_readme() {
    let report = lint::lint_path(&example());
    assert!(report.passed());
    // and the corpus really does carry the declarative config, not the old shell one
    assert!(example().join("verify.toml").is_file());
    assert!(!example().join("verify.config").exists());
}

#[test]
fn taskfile_round_trip() {
    let (text, readme) = example_text();
    let tf = TaskFile::parse(text.clone(), &readme).unwrap();
    assert_eq!(tf.frontmatter.schema, "task/v4");
    assert_eq!(tf.frontmatter.id, "TASK-042");
    assert_eq!(tf.frontmatter.verify, "taskfmt verify");
    assert_eq!(
        tf.frontmatter.expected_paths,
        vec!["src/auth/session/*", "tests/auth/*", "Cargo.lock"]
    );
    assert_eq!(tf.ac_rows.len(), 3);
    assert_eq!(
        tf.h1.as_deref(),
        Some("TASK-042 — Reject expired refresh tokens before session rotation")
    );
    // the gate leaf names the gate command
    assert!(
        tf.checklist
            .iter()
            .any(|line| line.contains("taskfmt verify"))
    );
}
