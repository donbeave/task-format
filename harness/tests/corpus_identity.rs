//! A package without a static base must receive an immutable base from the caller.

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

#[test]
fn missing_base_is_refused_without_a_lifecycle_record() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();
    git(root, &["init", "-q"]);
    git(root, &["config", "user.name", "taskfmt test"]);
    git(root, &["config", "user.email", "taskfmt@example.invalid"]);
    std::fs::write(root.join("tracked.txt"), "base\n").unwrap();
    git(root, &["add", "tracked.txt"]);
    git(root, &["commit", "-qm", "base"]);
    let task = tempfile::tempdir().unwrap();
    std::fs::write(
        task.path().join("verify.toml"),
        "schema = \"verify/v2\"\ntask_id = \"TASK-900\"\nwritable_paths = [\"tracked.txt\"]\n\n[[checks]]\nid = \"CHK-001\"\nphase = \"gate\"\nargv = [\"true\"]\nrequirements = [\"R-001\"]\nacceptance = [\"AC-001\"]\n",
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
    assert_eq!(result.exit, gate::EXIT_INTERNAL, "{}", result.text);
    assert!(result.text.contains("no immutable base"), "{}", result.text);
}
