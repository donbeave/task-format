//! Strict v5/v2 corpus lint contract.

use std::path::PathBuf;

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
    let mut packages = vec![example()];
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
