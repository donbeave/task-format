//! Regression proof for the static corpus identity foot-gun.
//!
//! A task's configured fallback base is consumed by the scope gate as a Git
//! commit (`git diff <base>`).  A tree object is not interchangeable with that
//! commit, even though both are hexadecimal object IDs.  Lifecycle dispatch
//! supplies its recorded base explicitly; this test protects direct gate use
//! from accepting an unusable static fallback.

use std::process::Command;

use taskfmt::gate::{self, GateOpts};

fn git(root: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(root)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?}");
}

fn output(root: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("run git");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn tree_object_is_not_a_usable_static_scope_base() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();
    git(root, &["init", "-q"]);
    git(root, &["config", "user.name", "taskfmt test"]);
    git(root, &["config", "user.email", "taskfmt@example.invalid"]);
    std::fs::write(root.join("tracked.txt"), "base\n").unwrap();
    git(root, &["add", "tracked.txt"]);
    git(root, &["commit", "-qm", "base"]);
    let tree = output(root, &["rev-parse", "HEAD^{tree}"]);

    let task = tempfile::tempdir().unwrap();
    std::fs::write(
        task.path().join("verify.toml"),
        format!(
            "schema = \"verify/v2\"\ntask_id = \"TASK-900\"\nbase_tree = \"{tree}\"\nwritable_paths = [\"tracked.txt\"]\n\n[[checks]]\nid = \"CHK-001\"\nphase = \"gate\"\nargv = [\"true\"]\nrequirements = [\"R-001\"]\nacceptance = [\"AC-001\"]\n"
        ),
    )
    .unwrap();

    let result = gate::run(GateOpts {
        root: root.to_path_buf(),
        task_dir: task.path().to_path_buf(),
        progress: None,
        base: None,
        log_dir: None,
        fail_fast: false,
        enforce_task_contract: false,
    });
    assert_eq!(result.exit, gate::EXIT_FAIL, "{}", result.text);
    assert!(
        result.text.contains("BASE_REF not resolvable"),
        "{}",
        result.text
    );
}
