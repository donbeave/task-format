use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn cli(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_taskfmt"))
        .args(args)
        .output()
        .expect("start taskfmt")
}

fn text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn task_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/example")
}

fn copy_task(parent: &Path) -> PathBuf {
    let task_dir = parent.join("task");
    fs::create_dir_all(&task_dir).unwrap();
    for name in ["README.md", "verify.toml"] {
        fs::copy(task_fixture().join(name), task_dir.join(name)).unwrap();
    }
    task_dir
}

fn git_output(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("start git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn git(root: &Path, args: &[&str]) {
    let _ = git_output(root, args);
}

fn base_workspace(parent: &Path) -> PathBuf {
    let root = parent.join("workspace");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/base.rs"), "// base\n").unwrap();
    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.name", "Taskfmt Test"]);
    git(
        &root,
        &["config", "user.email", "taskfmt-test@example.invalid"],
    );
    git(&root, &["add", "-A"]);
    git(
        &root,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-q",
            "-m",
            "base",
        ],
    );
    root
}

fn verify_args(root: &Path, task_dir: &Path, base: &str) -> Vec<String> {
    vec![
        "verify".into(),
        "--root".into(),
        root.display().to_string(),
        "--task-dir".into(),
        task_dir.display().to_string(),
        "--base".into(),
        base.into(),
    ]
}

fn progress(events: &str, state: &str, current: &str, latest: usize) -> String {
    format!(
        "---\nschema: progress/v1\ntask: TASK-042\nstate: {state}\ncurrent: {current}\nlatest_event: {latest}\n---\n\n## Events\n{events}\n\n## Handoff\nCURRENT_FAILURE: none\n"
    )
}

fn partial_progress() -> String {
    progress(
        "- 1 | STARTED | 1.1\n- 2 | DONE | 1.1\n- 3 | STARTED | 2.1\n- 4 | DONE | 2.1\n- 5 | STARTED | 2.2",
        "IN_PROGRESS",
        "2.2",
        5,
    )
}

fn blocked_progress() -> String {
    progress(
        "- 1 | STARTED | 1.1\n- 2 | BLOCKED | 1.1",
        "BLOCKED",
        "1.1",
        2,
    )
}

fn completed_progress() -> String {
    let events = ["1.1", "2.1", "2.2", "2.3", "3.1"]
        .iter()
        .flat_map(|leaf| [format!("STARTED | {leaf}"), format!("DONE | {leaf}")])
        .enumerate()
        .map(|(index, event)| format!("- {} | {event}", index + 1))
        .collect::<Vec<_>>()
        .join("\n");
    progress(&events, "DONE", "NONE", 10)
}

#[test]
fn lint_reports_json_and_never_executes_declared_commands() {
    let temp = TempDir::new().unwrap();
    let task_dir = copy_task(temp.path());
    let config_path = task_dir.join("verify.toml");
    let config = fs::read_to_string(&config_path).unwrap().replace(
        "argv = [\"true\"]",
        "argv = [\"taskfmt-command-that-does-not-exist\"]",
    );
    fs::write(config_path, config).unwrap();

    let output = cli(&[
        "lint".into(),
        task_dir.display().to_string(),
        "--json".into(),
    ]);
    assert!(output.status.success(), "{}", text(&output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "bad JSON {error}: stdout={} stderr={}",
            text(&output),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    assert_eq!(report["findings"], serde_json::json!([]));

    let missing = cli(&[
        "lint".into(),
        temp.path().join("missing").display().to_string(),
        "--json".into(),
    ]);
    assert_eq!(missing.status.code(), Some(1));
    let missing_report: Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert_eq!(missing_report["findings"][0]["rule"], "task_dir");
}

#[test]
fn status_reports_leaf_percentage_and_rejects_invalid_progress_paths() {
    let temp = TempDir::new().unwrap();
    let task_dir = copy_task(temp.path());
    let progress_path = temp.path().join("progress.md");
    fs::write(&progress_path, partial_progress()).unwrap();

    let output = cli(&[
        "status".into(),
        "--task-dir".into(),
        task_dir.display().to_string(),
        "--progress".into(),
        progress_path.display().to_string(),
        "--json".into(),
    ]);
    assert!(output.status.success(), "{}", text(&output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["state"], "IN_PROGRESS");
    assert_eq!(report["current"], "2.2");
    assert_eq!(report["completed_leaves"], 2);
    assert_eq!(report["total_leaves"], 5);
    assert_eq!(report["percent"], 40);

    fs::write(&progress_path, blocked_progress()).unwrap();
    let blocked = cli(&[
        "status".into(),
        "--task-dir".into(),
        task_dir.display().to_string(),
        "--progress".into(),
        progress_path.display().to_string(),
        "--json".into(),
    ]);
    assert!(blocked.status.success(), "{}", text(&blocked));
    let blocked_report: Value = serde_json::from_slice(&blocked.stdout).unwrap();
    assert_eq!(blocked_report["state"], "BLOCKED");
    assert_eq!(
        blocked_report["checklist"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == "1.1")
            .unwrap()["status"],
        "blocked"
    );

    fs::write(
        &progress_path,
        partial_progress().replace("task: TASK-042", "task: TASK-999"),
    )
    .unwrap();
    let wrong_task = cli(&[
        "status".into(),
        "--task-dir".into(),
        task_dir.display().to_string(),
        "--progress".into(),
        progress_path.display().to_string(),
    ]);
    assert_eq!(wrong_task.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&wrong_task.stderr).contains("does not match"));

    fs::write(
        &progress_path,
        partial_progress().replace("latest_event: 5", "latest_event: 6"),
    )
    .unwrap();
    let malformed = cli(&[
        "status".into(),
        "--task-dir".into(),
        task_dir.display().to_string(),
        "--progress".into(),
        progress_path.display().to_string(),
    ]);
    assert_eq!(malformed.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&malformed.stderr).contains("invalid or missing progress"),
        "stderr={}",
        String::from_utf8_lossy(&malformed.stderr)
    );

    let absent = cli(&[
        "status".into(),
        "--task-dir".into(),
        task_dir.display().to_string(),
        "--progress".into(),
        temp.path().join("absent.md").display().to_string(),
    ]);
    assert_eq!(absent.status.code(), Some(1));
}

#[test]
fn status_rejects_a_task_with_zero_checklist_leaves() {
    let temp = TempDir::new().unwrap();
    let task_dir = copy_task(temp.path());
    let readme_path = task_dir.join("README.md");
    let readme = fs::read_to_string(&readme_path).unwrap();
    let marker = "<!-- checklist:start -->";
    let start = readme.find(marker).unwrap() + marker.len();
    let end = readme.find("<!-- checklist:end -->").unwrap();
    fs::write(
        readme_path,
        format!("{}\n{}", &readme[..start], &readme[end..]),
    )
    .unwrap();

    let progress_path = temp.path().join("progress.md");
    fs::write(&progress_path, partial_progress()).unwrap();
    let output = cli(&[
        "status".into(),
        "--task-dir".into(),
        task_dir.display().to_string(),
        "--progress".into(),
        progress_path.display().to_string(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("empty checklist"));
}

#[test]
fn verify_checks_only_ends_with_checks_pass_and_full_completion_ends_with_done() {
    let temp = TempDir::new().unwrap();
    let root = base_workspace(temp.path());
    let task_dir = copy_task(temp.path());

    let mut checks_args = verify_args(&root, &task_dir, "HEAD");
    checks_args.push("--no-progress".into());
    let checks = cli(&checks_args);
    assert!(checks.status.success(), "{}", text(&checks));
    assert!(text(&checks).ends_with("CHECKS PASS\n"));
    assert!(!text(&checks).contains("\nDONE\n"));

    let progress_path = temp.path().join("progress.md");
    fs::write(&progress_path, completed_progress()).unwrap();
    let mut full_args = verify_args(&root, &task_dir, "HEAD");
    full_args.extend(["--progress".into(), progress_path.display().to_string()]);
    let full = cli(&full_args);
    assert!(full.status.success(), "{}", text(&full));
    assert!(text(&full).ends_with("DONE\n"));

    fs::write(&progress_path, partial_progress()).unwrap();
    let incomplete = cli(&full_args);
    assert_eq!(incomplete.status.code(), Some(1));
    assert!(text(&incomplete).contains("CHECK progress FAIL"));
    assert!(!text(&incomplete).ends_with("DONE\n"));
}

#[test]
fn verify_preserves_argv_shell_and_expected_result_checks() {
    let temp = TempDir::new().unwrap();
    let root = base_workspace(temp.path());
    let task_dir = copy_task(temp.path());
    let config_path = task_dir.join("verify.toml");
    let config = fs::read_to_string(&config_path)
        .unwrap()
        .replacen(
            "argv = [\"true\"]",
            "argv = [\"printf\", \"argv-value\"]\nexpected = { stdout_contains = [\"argv-value\"] }",
            1,
        )
        .replacen(
            "shell = \"true\"",
            "shell = \"printf 'shell-value\\\\n'\"\nexpected = { stdout_regex = [\"^shell-value$\"] }",
            1,
        );
    fs::write(config_path, config).unwrap();

    let mut args = verify_args(&root, &task_dir, "HEAD");
    args.push("--no-progress".into());
    let output = cli(&args);
    assert!(output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("CHECK CHK-001 PASS"));
    assert!(text(&output).contains("CHECK CHK-003 PASS"));
    assert!(text(&output).ends_with("CHECKS PASS\n"));
}

#[test]
fn verify_fails_when_an_expected_result_misses_even_if_the_command_exits_zero() {
    let temp = TempDir::new().unwrap();
    let root = base_workspace(temp.path());
    let task_dir = copy_task(temp.path());
    let config_path = task_dir.join("verify.toml");
    let config = fs::read_to_string(&config_path).unwrap().replacen(
        "argv = [\"true\"]",
        "argv = [\"true\"]\nexpected = { stdout_contains = [\"required-marker\"] }",
        1,
    );
    fs::write(config_path, config).unwrap();

    let mut args = verify_args(&root, &task_dir, "HEAD");
    args.push("--no-progress".into());
    let output = cli(&args);
    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output).contains("CHECK CHK-001 FAIL"));
    assert!(text(&output).contains("CHK-001 stdout lacks \"required-marker\""));
    assert!(!text(&output).ends_with("CHECKS PASS\n"));
}

#[test]
fn verify_enforces_base_tree_against_explicit_base_and_rejects_legacy_predecessor_field() {
    let temp = TempDir::new().unwrap();
    let root = base_workspace(temp.path());
    let pinned_commit = git_output(&root, &["rev-parse", "HEAD"]);
    fs::create_dir_all(root.join("src/auth/session")).unwrap();
    fs::write(
        root.join("src/auth/session/change.rs"),
        "// scoped change\n",
    )
    .unwrap();
    git(&root, &["add", "-A"]);
    git(
        &root,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-q",
            "-m",
            "scoped change",
        ],
    );

    let task_dir = copy_task(temp.path());
    let config_path = task_dir.join("verify.toml");
    let config = fs::read_to_string(&config_path).unwrap().replacen(
        "task_id = \"TASK-042\"",
        &format!("task_id = \"TASK-042\"\nbase_tree = \"{pinned_commit}\""),
        1,
    );
    fs::write(&config_path, config).unwrap();

    let mut matching_base = verify_args(&root, &task_dir, &pinned_commit);
    matching_base.push("--no-progress".into());
    let matching = cli(&matching_base);
    assert!(matching.status.success(), "{}", text(&matching));
    assert!(text(&matching).contains("CHECK base_tree PASS"));

    let mut different_base = verify_args(&root, &task_dir, "HEAD");
    different_base.push("--no-progress".into());
    let different = cli(&different_base);
    assert_eq!(different.status.code(), Some(1));
    assert!(text(&different).contains("CHECK base_tree FAIL"));
    assert!(text(&different).contains("but base_tree pins"));

    let legacy = fs::read_to_string(&config_path).unwrap().replace(
        &format!("base_tree = \"{pinned_commit}\""),
        &format!("base_tree = \"{pinned_commit}\"\npredecessor = {{ task_id = \"TASK-041\" }}"),
    );
    fs::write(config_path, legacy).unwrap();
    let lint = cli(&[
        "lint".into(),
        task_dir.display().to_string(),
        "--json".into(),
    ]);
    assert_eq!(lint.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&lint.stdout).unwrap();
    assert!(
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| {
                finding["rule"] == "config"
                    && finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("predecessor"))
            })
    );
}

#[test]
fn lint_reports_missing_verify_config_and_verify_reports_missing_executable() {
    let temp = TempDir::new().unwrap();
    let task_dir = copy_task(temp.path());
    fs::remove_file(task_dir.join("verify.toml")).unwrap();
    let lint = cli(&[
        "lint".into(),
        task_dir.display().to_string(),
        "--json".into(),
    ]);
    assert_eq!(lint.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&lint.stdout).unwrap();
    assert!(
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| {
                finding["rule"] == "config"
                    && finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("verify.toml invalid"))
            })
    );

    let root = base_workspace(temp.path());
    let executable_task = copy_task(&temp.path().join("with-command"));
    let config_path = executable_task.join("verify.toml");
    let config = fs::read_to_string(&config_path).unwrap().replacen(
        "argv = [\"true\"]",
        "argv = [\"taskfmt-command-that-does-not-exist\"]",
        1,
    );
    fs::write(config_path, config).unwrap();
    let mut args = verify_args(&root, &executable_task, "HEAD");
    args.push("--no-progress".into());
    let verify = cli(&args);
    assert_eq!(verify.status.code(), Some(1));
    assert!(text(&verify).contains("CHECK CHK-001 FAIL"));
    assert!(text(&verify).contains("CHK-001 could not start"));
}

#[test]
fn verify_requires_scope_base_and_rejects_invalid_paths_and_out_of_scope_changes() {
    let temp = TempDir::new().unwrap();
    let root = base_workspace(temp.path());
    let task_dir = copy_task(temp.path());

    let missing_base = cli(&[
        "verify".into(),
        "--root".into(),
        root.display().to_string(),
        "--task-dir".into(),
        task_dir.display().to_string(),
        "--no-progress".into(),
    ]);
    assert_eq!(missing_base.status.code(), Some(2));

    let mut bad_root = verify_args(&temp.path().join("no-workspace"), &task_dir, "HEAD");
    bad_root.push("--no-progress".into());
    let bad_root = cli(&bad_root);
    assert_eq!(bad_root.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&bad_root.stderr).contains("cannot resolve workspace root"));

    let mut bad_base = verify_args(&root, &task_dir, "missing-ref");
    bad_base.push("--no-progress".into());
    let bad_base = cli(&bad_base);
    assert_eq!(bad_base.status.code(), Some(1));
    assert!(text(&bad_base).contains("base ref does not resolve to a commit"));

    fs::write(root.join("outside.txt"), "outside writable scope\n").unwrap();
    let mut out_of_scope = verify_args(&root, &task_dir, "HEAD");
    out_of_scope.push("--no-progress".into());
    let out_of_scope = cli(&out_of_scope);
    assert_eq!(out_of_scope.status.code(), Some(1));
    assert!(text(&out_of_scope).contains("changed path is outside writable_paths: outside.txt"));
}
