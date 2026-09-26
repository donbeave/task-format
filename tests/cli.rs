use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::SystemTime;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

use serde_json::Value;
use tempfile::TempDir;

fn cli(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_taskfmt"))
        .args(args)
        .output()
        .expect("start taskfmt")
}

fn cli_with_env(args: &[String], key: &str, value: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_taskfmt"))
        .args(args)
        .env(key, value)
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

#[derive(Debug, PartialEq, Eq)]
enum SnapshotKind {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, PartialEq, Eq)]
struct SnapshotEntry {
    kind: SnapshotKind,
    contents: Option<Vec<u8>>,
    symlink_target: Option<PathBuf>,
    modified: Option<SystemTime>,
    permissions: u32,
}

#[cfg(unix)]
fn permission_bits(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode()
}

#[cfg(not(unix))]
fn permission_bits(metadata: &fs::Metadata) -> u32 {
    u32::from(metadata.permissions().readonly())
}

fn snapshot_entry(path: &Path) -> SnapshotEntry {
    let metadata = fs::symlink_metadata(path).unwrap();
    let file_type = metadata.file_type();
    let kind = if file_type.is_file() {
        SnapshotKind::File
    } else if file_type.is_dir() {
        SnapshotKind::Directory
    } else if file_type.is_symlink() {
        SnapshotKind::Symlink
    } else {
        SnapshotKind::Other
    };
    SnapshotEntry {
        contents: (kind == SnapshotKind::File).then(|| fs::read(path).unwrap()),
        symlink_target: (kind == SnapshotKind::Symlink).then(|| fs::read_link(path).unwrap()),
        modified: metadata.modified().ok(),
        permissions: permission_bits(&metadata),
        kind,
    }
}

fn snapshot_tree(root: &Path) -> Vec<(PathBuf, SnapshotEntry)> {
    fn visit(root: &Path, dir: &Path, snapshot: &mut Vec<(PathBuf, SnapshotEntry)>) {
        let mut entries = fs::read_dir(dir)
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            snapshot.push((
                path.strip_prefix(root).unwrap().to_path_buf(),
                snapshot_entry(&path),
            ));
            if entry.file_type().unwrap().is_dir() {
                visit(root, &path, snapshot);
            }
        }
    }

    let mut snapshot = vec![(PathBuf::new(), snapshot_entry(root))];
    visit(root, root, &mut snapshot);
    snapshot
}

#[test]
fn record_declared_command_execution_marker_when_requested() {
    if let Some(marker) = std::env::var_os("TASKFMT_TEST_DECLARED_COMMAND_MARKER") {
        fs::write(marker, b"executed").unwrap();
    }
}

fn marker_command_args() -> Vec<String> {
    vec![
        std::env::current_exe().unwrap().display().to_string(),
        "--exact".into(),
        "record_declared_command_execution_marker_when_requested".into(),
        "--nocapture".into(),
    ]
}

fn marker_command_argv() -> String {
    let args = marker_command_args()
        .iter()
        .map(|arg| serde_json::to_string(arg).unwrap())
        .collect::<Vec<_>>()
        .join(", ");
    format!("argv = [{args}]")
}

fn shell_quote(argument: &str) -> String {
    format!("'{}'", argument.replace('\'', "'\\''"))
}

fn marker_command_shell() -> String {
    marker_command_args()
        .iter()
        .map(|argument| shell_quote(argument))
        .collect::<Vec<_>>()
        .join(" ")
}

fn marker_command_shell_toml() -> String {
    format!(
        "shell = {}",
        serde_json::to_string(&marker_command_shell()).unwrap()
    )
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

    let config_path = task_dir.join("verify.toml");
    let original_config = fs::read_to_string(&config_path).unwrap();
    let config = original_config.replacen(
        "argv = [\"true\"]",
        &format!(
            "argv = [{}, \"__unknown_taskfmt_command__\"]",
            serde_json::to_string(env!("CARGO_BIN_EXE_taskfmt")).unwrap()
        ),
        1,
    );
    assert_ne!(
        config, original_config,
        "failing command fixture did not apply"
    );
    fs::write(config_path, config).unwrap();
    fs::write(&progress_path, completed_progress()).unwrap();
    let failing_check_with_done_progress = cli(&full_args);
    assert_eq!(failing_check_with_done_progress.status.code(), Some(1));
    assert!(text(&failing_check_with_done_progress).contains("CHECK CHK-001 FAIL"));
    assert!(!text(&failing_check_with_done_progress).ends_with("DONE\n"));
}

#[test]
fn canonical_template_runs_through_lint_status_and_both_verify_modes() {
    let temp = TempDir::new().unwrap();
    let task_id = "TASK-900";
    let template_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/task-template");
    let task_dir = temp.path().join("task");
    fs::create_dir_all(&task_dir).unwrap();

    let readme = fs::read_to_string(template_dir.join("README.md"))
        .unwrap()
        .replace("TASK-000", task_id);
    fs::write(task_dir.join("README.md"), readme).unwrap();
    let verify_config = fs::read_to_string(template_dir.join("verify.toml"))
        .unwrap()
        .replace("TASK-000", task_id)
        .replace("<precondition-command>", "true")
        .replace("<focused-test-command>", "true")
        .replace("<regression-command>", "true")
        .replace("<lint-command>", "true")
        .replace("<gate-command>", "true");
    fs::write(task_dir.join("verify.toml"), verify_config).unwrap();

    let progress_path = temp.path().join("progress.md");
    fs::copy(template_dir.join("progress.md"), &progress_path).unwrap();
    let leaves = ["1.1", "2.1", "2.2", "2.3", "2.4", "3.1"];
    let events = leaves
        .iter()
        .flat_map(|leaf| [format!("STARTED | {leaf}"), format!("DONE | {leaf}")])
        .enumerate()
        .map(|(index, event)| format!("- {} | {event}", index + 1))
        .collect::<Vec<_>>()
        .join("\n");
    let completed_progress = fs::read_to_string(&progress_path)
        .unwrap()
        .replace("task: TASK-000", &format!("task: {task_id}"))
        .replace("state: IN_PROGRESS", "state: DONE")
        .replace("current: 1.1", "current: NONE")
        .replace("latest_event: 1", "latest_event: 12")
        .replace("- 1 | STARTED | 1.1", &events);
    fs::write(&progress_path, completed_progress).unwrap();

    let root = base_workspace(temp.path());
    let base = git_output(&root, &["rev-parse", "HEAD"]);

    let lint = cli(&[
        "lint".into(),
        task_dir.display().to_string(),
        "--json".into(),
    ]);
    assert!(lint.status.success(), "{}", text(&lint));

    let status = cli(&[
        "status".into(),
        "--task-dir".into(),
        task_dir.display().to_string(),
        "--progress".into(),
        progress_path.display().to_string(),
        "--json".into(),
    ]);
    assert!(status.status.success(), "{}", text(&status));
    let status_report: Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status_report["state"], "DONE");
    assert_eq!(status_report["completed_leaves"], 6);
    assert_eq!(status_report["total_leaves"], 6);
    assert_eq!(status_report["percent"], 100);

    let mut checks_only_args = verify_args(&root, &task_dir, &base);
    checks_only_args.push("--no-progress".into());
    let checks_only = cli(&checks_only_args);
    assert!(checks_only.status.success(), "{}", text(&checks_only));
    assert!(text(&checks_only).ends_with("CHECKS PASS\n"));
    assert!(!text(&checks_only).contains("\nDONE\n"));

    let mut full_args = verify_args(&root, &task_dir, &base);
    full_args.extend(["--progress".into(), progress_path.display().to_string()]);
    let full = cli(&full_args);
    assert!(full.status.success(), "{}", text(&full));
    assert!(text(&full).ends_with("DONE\n"));
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
fn verify_does_not_run_declared_checks_when_base_tree_mismatches() {
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
    let config = fs::read_to_string(&config_path)
        .unwrap()
        .replacen(
            "task_id = \"TASK-042\"",
            &format!("task_id = \"TASK-042\"\nbase_tree = \"{pinned_commit}\""),
            1,
        )
        .replacen("argv = [\"true\"]", &marker_command_argv(), 1)
        .replacen("shell = \"true\"", &marker_command_shell_toml(), 1);
    fs::write(&config_path, config).unwrap();

    let lint = cli(&[
        "lint".into(),
        task_dir.display().to_string(),
        "--json".into(),
    ]);
    assert!(lint.status.success(), "{}", text(&lint));

    let marker_path = temp.path().join("declared-command-ran");
    let mut args = verify_args(&root, &task_dir, "HEAD");
    args.push("--no-progress".into());
    let output = cli_with_env(&args, "TASKFMT_TEST_DECLARED_COMMAND_MARKER", &marker_path);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output).contains("CHECK base_tree FAIL"),
        "{}",
        text(&output)
    );
    assert!(
        !text(&output).contains("CHECK CHK-001"),
        "{}",
        text(&output)
    );
    assert!(
        !marker_path.exists(),
        "base_tree failure ran a declared check"
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
    let mut missing_config_args = verify_args(&root, &task_dir, "HEAD");
    missing_config_args.push("--no-progress".into());
    let missing_config = cli(&missing_config_args);
    assert_eq!(missing_config.status.code(), Some(70));
    assert!(text(&missing_config).contains("cannot load"));
    assert!(text(&missing_config).contains("verify.toml"));

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
fn lint_rejects_broken_acceptance_check_reference() {
    let temp = TempDir::new().unwrap();
    let task_dir = copy_task(temp.path());
    let readme_path = task_dir.join("README.md");
    let original_readme = fs::read_to_string(&readme_path).unwrap();
    let readme = original_readme.replacen("- **Check:** `CHK-001`", "- **Check:** `CHK-999`", 1);
    assert_ne!(
        readme, original_readme,
        "broken check reference was not applied"
    );
    fs::write(readme_path, readme).unwrap();

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
                finding["rule"] == "graph"
                    && finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("unknown check CHK-999"))
            })
    );
}

#[test]
fn lint_and_status_are_repeatable_read_only_and_do_not_execute_task_commands() {
    let temp = TempDir::new().unwrap();
    let task_dir = copy_task(temp.path());
    let config_path = task_dir.join("verify.toml");
    let original_config = fs::read_to_string(&config_path).unwrap();
    let config = original_config
        .replacen("argv = [\"true\"]", &marker_command_argv(), 1)
        .replacen("shell = \"true\"", &marker_command_shell_toml(), 1);
    assert_ne!(
        config, original_config,
        "marker command fixture did not apply"
    );
    fs::write(config_path, config).unwrap();
    let progress_path = temp.path().join("progress.md");
    fs::write(&progress_path, partial_progress()).unwrap();
    let marker_path = temp.path().join("declared-command-ran");

    let command = marker_command_args();
    let marker_probe = Command::new(&command[0])
        .args(&command[1..])
        .env("TASKFMT_TEST_DECLARED_COMMAND_MARKER", &marker_path)
        .output()
        .unwrap();
    assert!(
        marker_probe.status.success(),
        "marker probe failed: {}",
        String::from_utf8_lossy(&marker_probe.stderr)
    );
    assert_eq!(fs::read(&marker_path).unwrap(), b"executed");
    fs::remove_file(&marker_path).unwrap();

    let shell_probe = Command::new("bash")
        .args(["-eo", "pipefail", "-c"])
        .arg(marker_command_shell())
        .env("TASKFMT_TEST_DECLARED_COMMAND_MARKER", &marker_path)
        .output()
        .unwrap();
    assert!(
        shell_probe.status.success(),
        "shell marker probe failed: {}",
        String::from_utf8_lossy(&shell_probe.stderr)
    );
    assert_eq!(fs::read(&marker_path).unwrap(), b"executed");
    fs::remove_file(&marker_path).unwrap();

    let before = snapshot_tree(temp.path());

    let lint_args = vec![
        "lint".into(),
        task_dir.display().to_string(),
        "--json".into(),
    ];
    let status_args = vec![
        "status".into(),
        "--task-dir".into(),
        task_dir.display().to_string(),
        "--progress".into(),
        progress_path.display().to_string(),
        "--json".into(),
    ];

    let first_lint = cli_with_env(
        &lint_args,
        "TASKFMT_TEST_DECLARED_COMMAND_MARKER",
        &marker_path,
    );
    let second_lint = cli_with_env(
        &lint_args,
        "TASKFMT_TEST_DECLARED_COMMAND_MARKER",
        &marker_path,
    );
    assert!(first_lint.status.success(), "{}", text(&first_lint));
    assert_eq!(first_lint.stdout, second_lint.stdout);
    assert_eq!(first_lint.stderr, second_lint.stderr);
    assert_eq!(first_lint.status, second_lint.status);

    let first_status = cli_with_env(
        &status_args,
        "TASKFMT_TEST_DECLARED_COMMAND_MARKER",
        &marker_path,
    );
    let second_status = cli_with_env(
        &status_args,
        "TASKFMT_TEST_DECLARED_COMMAND_MARKER",
        &marker_path,
    );
    assert!(first_status.status.success(), "{}", text(&first_status));
    assert_eq!(first_status.stdout, second_status.stdout);
    assert_eq!(first_status.stderr, second_status.stderr);
    assert_eq!(first_status.status, second_status.status);

    assert!(!marker_path.exists(), "lint or status ran a task command");
    assert_eq!(
        snapshot_tree(temp.path()),
        before,
        "lint or status changed files"
    );
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

#[test]
fn verify_scope_reports_untracked_paths_hidden_by_a_new_root_gitignore() {
    let temp = TempDir::new().unwrap();
    let root = base_workspace(temp.path());
    let task_dir = copy_task(temp.path());

    fs::write(root.join(".gitignore"), "outside.txt\n").unwrap();
    fs::write(root.join("outside.txt"), "outside writable scope\n").unwrap();

    let mut args = verify_args(&root, &task_dir, "HEAD");
    args.push("--no-progress".into());
    let output = cli(&args);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output).contains("CHECK scope FAIL"),
        "{}",
        text(&output)
    );
    assert!(
        text(&output).contains("changed path is outside writable_paths: outside.txt"),
        "{}",
        text(&output)
    );
}

#[test]
fn verify_rechecks_scope_after_declared_commands_against_immutable_base() {
    let temp = TempDir::new().unwrap();
    let root = base_workspace(temp.path());
    let task_dir = copy_task(temp.path());
    let config_path = task_dir.join("verify.toml");
    let command = "mkdir -p src/auth/session && printf forbidden > src/auth/session/legacy_expiry_check.rs && printf outside > outside.txt && git add -A && git commit -q -m declared-command";
    let config = fs::read_to_string(&config_path).unwrap().replacen(
        "argv = [\"true\"]",
        &format!("shell = {}", serde_json::to_string(command).unwrap()),
        1,
    );
    fs::write(config_path, config).unwrap();

    let mut args = verify_args(&root, &task_dir, "HEAD");
    args.push("--no-progress".into());
    let output = cli(&args);
    let report = text(&output);

    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(report.contains("CHECK CHK-001 PASS"), "{report}");
    assert!(report.contains("CHECK scope FAIL"), "{report}");
    assert!(
        report.contains("changed path is outside writable_paths: outside.txt"),
        "{report}"
    );
    assert!(report.contains("CHECK forbidden_paths FAIL"), "{report}");
    assert!(
        report.contains("forbidden path changed: src/auth/session/legacy_expiry_check.rs"),
        "{report}"
    );
}

#[test]
fn verify_rechecks_artifacts_after_later_declared_commands() {
    let temp = TempDir::new().unwrap();
    let root = base_workspace(temp.path());
    let task_dir = copy_task(temp.path());
    let config_path = task_dir.join("verify.toml");
    let original = fs::read_to_string(&config_path).unwrap();
    let config = original
        .replacen(
            "id = \"CHK-001\"\nphase = \"focused\"\nargv = [\"true\"]",
            "id = \"CHK-001\"\nphase = \"focused\"\nargv = [\"true\"]\nexpected = { forbidden_artifacts = [\"src/auth/session/late.txt\"] }",
            1,
        )
        .replacen(
            "id = \"CHK-002\"\nphase = \"regression\"\nargv = [\"true\"]",
            "id = \"CHK-002\"\nphase = \"regression\"\nshell = \"mkdir -p src/auth/session && printf late > src/auth/session/late.txt\"",
            1,
        );
    assert_ne!(config, original);
    fs::write(config_path, config).unwrap();

    let mut args = verify_args(&root, &task_dir, "HEAD");
    args.push("--no-progress".into());
    let output = cli(&args);
    let report = text(&output);

    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(report.contains("CHECK CHK-001 FAIL"), "{report}");
    assert!(
        report.contains(
            "forbidden artifact exists: src/auth/session/late.txt after all declared checks"
        ),
        "{report}"
    );
}

#[test]
fn lint_rejects_checks_after_the_completion_gate() {
    let temp = TempDir::new().unwrap();
    let task_dir = copy_task(temp.path());
    let config_path = task_dir.join("verify.toml");
    let mut config = fs::read_to_string(&config_path).unwrap().replacen(
        "phase = \"focused\"",
        "phase = \"gate\"",
        1,
    );
    let last_gate = config.rfind("phase = \"gate\"").unwrap();
    config.replace_range(
        last_gate..last_gate + "phase = \"gate\"".len(),
        "phase = \"lint\"",
    );
    fs::write(config_path, config).unwrap();

    let output = cli(&[
        "lint".into(),
        task_dir.display().to_string(),
        "--json".into(),
    ]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| {
                finding["rule"] == "config"
                    && finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("checks must be ordered"))
            }),
        "{}",
        text(&output)
    );
}

#[cfg(unix)]
#[test]
fn verify_scope_fails_closed_for_invalid_utf8_path_near_valid_replacement_character_path() {
    let temp = TempDir::new().unwrap();
    let root = base_workspace(temp.path());
    let task_dir = copy_task(temp.path());

    fs::write(root.join(".gitignore"), "\u{fffd}\n").unwrap();
    git(&root, &["add", ".gitignore"]);
    git(
        &root,
        &[
            "commit",
            "-q",
            "-m",
            "ignore replacement-character filename",
        ],
    );

    fs::write(root.join("\u{fffd}"), "ignored valid UTF-8 filename\n").unwrap();
    let invalid_path = root.join(std::ffi::OsStr::from_bytes(&[0xff]));
    if let Err(error) = fs::write(&invalid_path, "invalid UTF-8 filename\n") {
        #[cfg(target_os = "macos")]
        let filesystem_rejects_invalid_name = error.raw_os_error() == Some(92); // EILSEQ
        #[cfg(not(target_os = "macos"))]
        let filesystem_rejects_invalid_name = error.kind() == std::io::ErrorKind::InvalidInput;
        assert!(
            filesystem_rejects_invalid_name,
            "unexpected failure creating invalid UTF-8 filename: {error}"
        );
        eprintln!("skipping: filesystem rejects invalid UTF-8 filenames: {error}");
        return;
    }

    let mut args = verify_args(&root, &task_dir, "HEAD");
    args.push("--no-progress".into());
    let output = cli(&args);
    let report = format!(
        "{}\n{}",
        text(&output),
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(report.contains("CHECK scope FAIL"), "{report}");
    assert!(
        report.to_ascii_lowercase().contains("not valid utf-8"),
        "expected a fail-closed non-UTF-8 path diagnostic, got:\n{report}"
    );
}
