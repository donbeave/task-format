//! Strict v5/v2 corpus lint contract.

use std::path::PathBuf;
use std::process::Command;

use taskfmt::lint::{self, Finding, Severity};
use taskfmt::taskfile::TaskFile;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("harness has repository parent")
        .to_path_buf()
}

fn example() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/example")
}

fn example_text() -> (String, PathBuf) {
    let readme = example().join("README.md");
    (std::fs::read_to_string(&readme).unwrap(), readme)
}

fn example_verify() -> String {
    std::fs::read_to_string(example().join("verify.toml")).unwrap()
}

fn lint_with_verify(readme: &str, verify: &str) -> Vec<Finding> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("README.md");
    std::fs::write(&path, readme).unwrap();
    std::fs::write(dir.path().join("verify.toml"), verify).unwrap();
    lint::lint_text(readme, &path)
}

fn has(findings: &[Finding], rule: &str, needle: &str) -> bool {
    findings.iter().any(|finding| {
        finding.severity == Severity::Error
            && finding.rule == rule
            && finding.message.contains(needle)
    })
}

fn mutate(text: &str, from: &str, to: &str) -> String {
    assert_eq!(text.matches(from).count(), 1, "mutation anchor: {from:?}");
    text.replacen(from, to, 1)
}

#[test]
fn corpus_and_example_lint_clean() {
    let root = repo_root();
    let mut packages = vec![example(), root.join("reference/task-template")];
    packages.extend((1..=7).map(|id| root.join(format!("experiments/tasks/TASK-{id:03}"))));
    for package in packages {
        let report = lint::lint_path(&package);
        assert_eq!(
            report.errors(),
            0,
            "{}:\n{}",
            package.display(),
            report.render()
        );
        assert_eq!(
            report.warnings(),
            0,
            "{}:\n{}",
            package.display(),
            report.render()
        );
    }
}

#[test]
fn diagnostics_have_stable_locations_and_json_shape() {
    let (text, readme) = example_text();
    let broken = mutate(&text, "kind: bugfix", "kind: bugfix\nverify: obsolete");
    let findings = lint::lint_text(&broken, &readme);
    assert_eq!(findings.len(), 1, "{findings:?}");
    let finding = &findings[0];
    assert_eq!(finding.rule, "frontmatter");
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(finding.path, readme);
    assert_eq!((finding.line, finding.column), (6, 1));

    let report = lint::LintReport {
        target: finding.path.clone(),
        findings,
    };
    let json: serde_json::Value = serde_json::from_str(&report.render_json()).unwrap();
    let item = &json["findings"][0];
    assert_eq!(item["rule"], "frontmatter");
    assert_eq!(item["severity"], "error");
    assert_eq!(item["line"], 6);
    assert_eq!(item["column"], 1);
    let text = report.render();
    assert!(text.contains("ERROR frontmatter "));
    assert!(text.contains(":6:1:"));
}

#[test]
fn diagnostics_attach_invalid_verifier_to_toml() {
    let (text, _) = example_text();
    let verify = example_verify().replace("schema = \"verify/v2\"", "schema = \"verify/v1\"");
    let findings = lint_with_verify(&text, &verify);
    let config = findings
        .iter()
        .find(|finding| finding.rule == "config")
        .unwrap();
    assert_eq!(config.path.file_name().unwrap(), "verify.toml");
    assert_eq!((config.line, config.column), (1, 1));
}

#[test]
fn semantic_diagnostics_name_the_owning_markdown_and_toml_fields() {
    let (text, readme) = example_text();
    // `scenario` also occurs in the document's prose. The diagnostic must use the Type field,
    // not the first matching word or the AC heading.
    let broken = text.replacen("- **Type:** scenario", "- **Type:** banana", 1);
    let finding = lint::lint_text(&broken, &readme)
        .into_iter()
        .find(|finding| finding.message.contains("Type must be"))
        .unwrap();
    assert_eq!((finding.line, finding.column), (55, 1));

    let verify = example_verify().replace("task_id = \"TASK-042\"", "task_id = \"bad\"");
    let finding = lint_with_verify(&text, &verify)
        .into_iter()
        .find(|finding| finding.rule == "config")
        .unwrap();
    assert_eq!(finding.path.file_name().unwrap(), "verify.toml");
    assert_eq!((finding.line, finding.column), (2, 1));
}

#[test]
fn frontmatter_unknown_and_duplicate_keys_are_fatal() {
    let (text, readme) = example_text();
    let unknown = mutate(
        &text,
        "kind: bugfix",
        "kind: bugfix\nverify: taskfmt verify",
    );
    let findings = lint::lint_text(&unknown, &readme);
    assert!(
        has(
            &findings,
            "frontmatter",
            "unknown or malformed key `verify`"
        ),
        "{findings:?}"
    );
    let duplicate = mutate(&text, "kind: bugfix", "kind: bugfix\nkind: bugfix");
    let findings = lint::lint_text(&duplicate, &readme);
    assert!(
        has(&findings, "frontmatter", "duplicate key `kind`"),
        "{findings:?}"
    );
}

#[test]
fn legacy_acceptance_is_fatal() {
    let (text, readme) = example_text();
    let legacy = mutate(
        &text,
        "## Acceptance criteria",
        "## Acceptance criteria\n\n| AC-999 | old table dialect |",
    );
    let findings = lint::lint_text(&legacy, &readme);
    assert!(
        has(
            &findings,
            "acceptance",
            "legacy table acceptance is forbidden"
        ),
        "{findings:?}"
    );
}

#[test]
fn graph_unknown_unused_and_missing_references_are_fatal() {
    let (text, _) = example_text();
    let verify = example_verify();
    let unknown = mutate(
        &text,
        "    - [ ] **2.2** Preserve valid-token rotation. (`R-004`, `AC-002`, `CHK-002`)",
        "    - [ ] **2.2** Preserve valid-token rotation. (`R-004`, `AC-002`, `CHK-999`)",
    );
    let findings = lint_with_verify(&unknown, &verify);
    assert!(
        has(
            &findings,
            "graph",
            "leaf 2.2 references unknown check CHK-999"
        ),
        "{findings:?}"
    );
    let unused = format!(
        "{verify}\n[[checks]]\nid = \"CHK-999\"\nphase = \"focused\"\nargv = [\"true\"]\nrequirements = [\"R-001\"]\nacceptance = [\"AC-001\"]\n"
    );
    let findings = lint_with_verify(&text, &unused);
    assert!(
        has(&findings, "graph", "unused/uncovered check CHK-999"),
        "{findings:?}"
    );
    let missing = mutate(&verify, "requirements = [\"R-004\"]", "requirements = []");
    let findings = lint_with_verify(&text, &missing);
    assert!(
        has(&findings, "graph", "CHK-002 has no requirements"),
        "{findings:?}"
    );
}

#[test]
fn empty_verifier_is_fatal() {
    let (text, _) = example_text();
    let empty = example_verify()
        .split("[[checks]]")
        .next()
        .unwrap()
        .to_string();
    let findings = lint_with_verify(&text, &empty);
    assert!(
        has(&findings, "config", "missing field `checks`")
            || has(&findings, "config", "checks empty"),
        "{findings:?}"
    );
}

#[test]
fn parsing_and_rendering_are_deterministic() {
    let (text, readme) = example_text();
    let first = TaskFile::parse(text.clone(), &readme).unwrap();
    let second = TaskFile::parse(text.clone(), &readme).unwrap();
    assert_eq!(first.frontmatter, second.frontmatter);
    assert_eq!(first.sections, second.sections);
    assert_eq!(first.checklist, second.checklist);
    assert_eq!(
        format!("{:?}", lint::lint_text(&text, &readme)),
        format!("{:?}", lint::lint_text(&text, &readme))
    );
}

#[test]
fn lint_cli_keeps_batch_and_ndjson_reports_attributable() {
    let temp = tempfile::tempdir().unwrap();
    let tasks = temp.path().join("tasks");
    std::fs::create_dir_all(&tasks).unwrap();
    for id in ["TASK-101", "TASK-102"] {
        let package = tasks.join(id);
        std::fs::create_dir_all(&package).unwrap();
        std::fs::copy(example().join("README.md"), package.join("README.md")).unwrap();
        std::fs::copy(example().join("verify.toml"), package.join("verify.toml")).unwrap();
    }
    let bad = tasks.join("TASK-102/README.md");
    let readme = std::fs::read_to_string(&bad).unwrap();
    std::fs::write(&bad, readme.replacen("kind: bugfix", "kind: nope", 1)).unwrap();
    let manifest = temp.path().join("experiment.toml");
    std::fs::write(
        &manifest,
        "schema = \"experiment/v1\"\n[paths]\ntasks_dir = \"tasks\"\n[agents.default]\nprofile = \"p\"\n[agents.profiles.p]\nkind = \"codex\"\nimage = \"image\"\n",
    )
    .unwrap();

    let run = |json: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_taskfmt"));
        command.arg("--config").arg(&manifest).arg("lint");
        if json {
            command.arg("--json");
        }
        command.output().unwrap()
    };
    let text = run(false);
    assert_eq!(text.status.code(), Some(1));
    let stdout = String::from_utf8(text.stdout).unwrap();
    assert!(stdout.contains("PACKAGE "));
    assert_eq!(stdout.matches("PACKAGE ").count(), 2);
    let json = run(true);
    assert_eq!(json.status.code(), Some(0), "JSON mode is streamable");
    let records: Vec<serde_json::Value> = String::from_utf8(json.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|record| record["target"].is_string()));
    assert!(records.iter().all(|record| record["findings"].is_array()));
}
