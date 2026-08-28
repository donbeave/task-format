//! Lint corpus: the v4 example must pass, broken variants must not, the template must not.

use std::path::{Path, PathBuf};

use taskfmt::lint::{self, Severity};
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

fn lint_text(text: &str, readme: &Path) -> Vec<lint::Finding> {
    lint::lint_text(text, readme)
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
}

#[test]
fn template_still_has_placeholders() {
    let report = lint::lint_path(&repo_root().join("reference/task-template"));
    assert!(
        !report.passed(),
        "the authoring template is not a task package"
    );
}

#[test]
fn frontmatter_contract() {
    let readme = example().join("README.md");
    let text = std::fs::read_to_string(&readme).unwrap();
    let findings = lint_text(&text, &readme);
    assert!(findings.is_empty(), "{findings:?}");

    // wrong schema
    let bad = text.replacen("schema: task/v4", "schema: task/v3", 1);
    let findings = lint_text(&bad, &readme);
    assert!(
        findings
            .iter()
            .any(|f| f.rule == "frontmatter" && f.message.contains("task/v4")),
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
    let readme = example().join("README.md");
    let text = std::fs::read_to_string(&readme).unwrap();

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
        findings
            .iter()
            .any(|f| f.rule == "checklist" && f.message.contains("evidence")),
        "{findings:?}"
    );

    // a non-contiguous id: 3.3 without 3.2
    let bad = text
        .lines()
        .filter(|l| !l.contains("**3.2**"))
        .collect::<Vec<_>>()
        .join("\n");
    let findings = lint_text(&bad, &readme);
    assert!(
        findings.iter().any(|f| f.rule == "checklist"),
        "{findings:?}"
    );

    // a broken id token
    let bad = text.replace("**3.2**", "**3.2X**");
    let findings = lint_text(&bad, &readme);
    assert!(
        findings
            .iter()
            .any(|f| f.rule == "checklist" && f.message.contains("bad line")),
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

#[test]
fn size_warns_but_does_not_error() {
    let readme = example().join("README.md");
    let text = std::fs::read_to_string(&readme).unwrap();
    let long_text = format!(
        "{text}\n{}",
        "filler line to exceed the size budget\n".repeat(400)
    );
    let findings = lint_text(&long_text, &readme);
    assert!(
        findings
            .iter()
            .any(|f| f.rule == "size" && f.severity == Severity::Warn),
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
    let readme = example().join("README.md");
    let text = std::fs::read_to_string(&readme).unwrap();
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
