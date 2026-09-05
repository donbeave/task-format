//! The full gate tamper matrix: fresh progress fails, the DONE file passes with `DONE`, and every
//! single tamper fails. Runs against a throwaway git workspace, no network.

use std::path::{Path, PathBuf};
use std::process::Command;

use taskfmt::gate::{self, GateOpts};
use taskfmt::ops;
use taskfmt::taskfile;

fn example() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/example")
}

const VERIFY_TOML: &str = r#"schema = "verify/v2"
task_id = "TASK-042"
base_tree = "792edca45d6c5c8570357d3864ad58d1f080d196"
writable_paths = ["*"]

[[checks]]
id = "CHK-001"
phase = "gate"
argv = ["true"]
"#;

fn git(dir: &Path, args: &[&str]) {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir).args(args);
    let captured = ops::capture(&mut cmd).unwrap();
    assert!(captured.ok(), "git {:?}: {}", args, captured.stderr);
}

fn workspace(dir: &Path) -> PathBuf {
    let work = dir.join("work");
    std::fs::create_dir_all(&work).unwrap();
    git(&work, &["init", "-q", "."]);
    git(
        &work,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "baseline",
        ],
    );
    git(&work, &["tag", "baseline"]);
    work
}

fn fixture(dir: &Path) -> (PathBuf, PathBuf) {
    let work = workspace(dir);
    let task = dir.join("task");
    std::fs::create_dir_all(&task).unwrap();
    std::fs::copy(example().join("README.md"), task.join("README.md")).unwrap();
    std::fs::write(task.join("verify.toml"), VERIFY_TOML).unwrap();
    (work, task)
}

fn gate_at(work: &Path, task: &Path, logs: &Path, progress: &Path) -> gate::GateOutput {
    gate::run(GateOpts {
        root: work.to_path_buf(),
        task_dir: task.to_path_buf(),
        progress: Some(progress.display().to_string()),
        base: Some("baseline".to_string()),
        log_dir: Some(logs.to_path_buf()),
        fail_fast: false,
        enforce_task_contract: false,
    })
}

fn to_done(progress: &str) -> String {
    let done = progress.replace("- [ ]", "- [x]");
    done.lines()
        .map(|line| {
            if line.starts_with("STATE: ") {
                "STATE: DONE".to_string()
            } else if line.starts_with("CURRENT: ") {
                "CURRENT: NONE".to_string()
            } else if line.starts_with("BASELINE: ") {
                "BASELINE: cargo test -p auth expired_refresh_token -> 1 failed".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// A gate pass is exactly: exit 0 and the last stdout line is `DONE`.
#[test]
fn pass_means_exit_zero_and_done() {
    let tmp = tempfile::tempdir().unwrap();
    let (work, task) = fixture(tmp.path());
    let logs = tmp.path().join("logs");
    let progress = tmp.path().join("progress.md");

    let generated =
        taskfmt::cmds::progress_init::generate_and_write(&example(), Some(&progress)).unwrap();
    assert_eq!(generated, 0);
    let fresh = std::fs::read_to_string(&progress).unwrap();
    assert_eq!(
        taskfmt::progress::header_value(&fresh, "STATE").as_deref(),
        Some("IN_PROGRESS")
    );
    assert_eq!(
        taskfmt::progress::header_value(&fresh, "CURRENT").as_deref(),
        Some("1.1")
    );
    assert_eq!(
        taskfmt::progress::header_value(&fresh, "BASELINE").as_deref(),
        Some("<not run>")
    );

    let output = gate_at(&work, &task, &logs, &progress);
    assert!(
        !output.is_pass(),
        "fresh progress must fail:\n{}",
        output.text
    );
    assert_eq!(output.failed_checks, vec!["progress"]);

    let done_path = tmp.path().join("done.md");
    std::fs::write(&done_path, to_done(&fresh)).unwrap();
    let output = gate_at(&work, &task, &logs, &done_path);
    assert!(
        output.is_pass(),
        "done progress must pass:\n{}",
        output.text
    );
    assert_eq!(output.last_line, "DONE");
    assert_eq!(output.exit, 0);
}

#[test]
fn every_tamper_fails_the_gate() {
    let tmp = tempfile::tempdir().unwrap();
    let (work, task) = fixture(tmp.path());
    let logs = tmp.path().join("logs");
    let fresh = tmp.path().join("fresh.md");
    taskfmt::cmds::progress_init::generate_and_write(&example(), Some(&fresh)).unwrap();
    let done = to_done(&std::fs::read_to_string(&fresh).unwrap());

    let done_line = |marker: &str| -> String {
        done.lines()
            .find(|line| line.contains(marker))
            .unwrap_or_else(|| panic!("no line containing {marker}"))
            .to_string()
    };

    let tamper = |name: &str, text: String| {
        let path = tmp.path().join(format!("{name}.md"));
        std::fs::write(&path, text).unwrap();
        let output = gate_at(&work, &task, &logs, &path);
        assert!(
            !output.is_pass(),
            "{name} must fail the gate:\n{}",
            output.text
        );
        assert_eq!(output.last_line, "RESULT FAIL", "{name}");
    };

    tamper(
        "state",
        done.replacen("STATE: DONE", "STATE: IN_PROGRESS", 1),
    );
    tamper("current", done.replacen("CURRENT: NONE", "CURRENT: 4.2", 1));
    tamper(
        "baseline",
        done.lines()
            .map(|line| {
                if line.starts_with("BASELINE: ") {
                    "BASELINE: <not run>".to_string()
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );
    tamper(
        "task-id",
        done.replacen("TASK: TASK-042", "TASK: TASK-099", 1),
    );

    // a checked parent with an unchecked child
    let line = done_line("**3**");
    tamper(
        "parent-checked",
        done.replace(&line, &line.replace("- [x]", "- [ ]")),
    );
    // an unchecked parent whose children are all done
    let line = done_line("**2** Implement.");
    tamper(
        "parent-unchecked",
        done.replace(&line, &line.replace("- [x]", "- [ ]")),
    );
    // reworded checklist text
    tamper("reworded", done.replace("**3.1**", "**3.1** (reworded)"));
    // deleted checklist line
    tamper(
        "deleted",
        done.lines()
            .filter(|line| !line.contains("**2.2**"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    // added checklist line
    tamper(
        "added",
        done.replace(
            "<!-- checklist:end -->",
            "    - [x] **4.3** extra — evidence: none.\n<!-- checklist:end -->",
        ),
    );

    // missing progress file
    let output = gate_at(&work, &task, &logs, &tmp.path().join("nope.md"));
    assert!(!output.is_pass());
}

#[test]
fn scope_whitelist_and_base_resolution() {
    let tmp = tempfile::tempdir().unwrap();
    let work = workspace(tmp.path());
    let task = tmp.path().join("task");
    std::fs::create_dir_all(&task).unwrap();
    std::fs::copy(example().join("README.md"), task.join("README.md")).unwrap();
    let logs = tmp.path().join("logs");
    let progress = tmp.path().join("progress.md");
    taskfmt::cmds::progress_init::generate_and_write(&example(), Some(&progress)).unwrap();
    std::fs::write(
        &progress,
        to_done(&std::fs::read_to_string(&progress).unwrap()),
    )
    .unwrap();

    let config = |paths: &str| {
        format!(
            "schema = \"verify/v2\"\ntask_id = \"TASK-042\"\nbase_tree = \"792edca45d6c5c8570357d3864ad58d1f080d196\"\nwritable_paths = {paths}\n\n[[checks]]\nid = \"CHK-001\"\nphase = \"gate\"\nargv = [\"true\"]\n"
        )
    };
    let gate_with = |config: &str| {
        std::fs::write(task.join("verify.toml"), config).unwrap();
        gate_at(&work, &task, &logs, &progress)
    };

    // a change inside the whitelist passes scope
    std::fs::write(work.join("kept.rs"), "ok\n").unwrap();
    let output = gate_with(&config("[\"kept.rs\"]"));
    assert!(
        !output.failed_checks.contains(&"scope".to_string()),
        "in-scope file flagged:\n{}",
        output.text
    );

    // a change outside it fails scope
    let output = gate_with(&config("[\"other/**\"]"));
    assert!(
        output.failed_checks.contains(&"scope".to_string()),
        "{}",
        output.text
    );

    // an empty writable-path list fails closed at config validation.
    let output = gate_with(&config("[]"));
    assert_eq!(output.exit, taskfmt::gate::EXIT_INTERNAL, "{}", output.text);

    // an unresolvable base fails scope
    std::fs::write(
        task.join("verify.toml"),
        config("[\"*\"]").replace(
            "base_tree = \"792edca45d6c5c8570357d3864ad58d1f080d196\"",
            "base_tree = \"ffffffffffffffffffffffffffffffffffffffff\"",
        ),
    )
    .unwrap();
    let output = gate::run(GateOpts {
        root: work.to_path_buf(),
        task_dir: task.to_path_buf(),
        progress: Some(progress.display().to_string()),
        // no --base here: base_tree must be the one that resolves
        base: None,
        log_dir: Some(logs.to_path_buf()),
        fail_fast: false,
        enforce_task_contract: false,
    });
    assert!(
        output.failed_checks.contains(&"scope".to_string()),
        "{}",
        output.text
    );

    // base precedence: --base beats TASKFMT_BASE beats exact base_tree.
    let cfg = taskfmt::verifycfg::VerifyConfig::parse(
        "schema = \"verify/v2\"\ntask_id = \"TASK-001\"\nbase_tree = \"0123456789012345678901234567890123456789\"\nwritable_paths = [\"src\"]\n[[checks]]\nid = \"CHK-001\"\nphase = \"gate\"\nargv = [\"true\"]\n",
    )
    .unwrap();
    assert_eq!(gate::resolve_base(&Some("flag".into()), &cfg), "flag");
    assert_eq!(
        gate::resolve_base_from(&None, Some("fromenv"), &cfg),
        "fromenv"
    );
    assert_eq!(
        gate::resolve_base_from(&None, None, &cfg),
        "0123456789012345678901234567890123456789"
    );
    assert_eq!(
        gate::resolve_base_from(&None, Some(""), &cfg),
        "0123456789012345678901234567890123456789"
    );
}

/// forbidden_paths means "not created or modified by this run": a trusted file that exists on the
/// base commit is fine while untouched, and fails the moment it is changed.
#[test]
fn forbidden_paths_reject_changes_not_existence() {
    let tmp = tempfile::tempdir().unwrap();
    let work = workspace(tmp.path());
    std::fs::create_dir_all(work.join("crates/pgtui/src")).unwrap();
    std::fs::write(work.join("crates/pgtui/src/render.rs"), "planner\n").unwrap();
    git(&work, &["add", "-A"]);
    git(
        &work,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "-m",
            "trusted",
        ],
    );
    git(&work, &["tag", "-f", "baseline"]);

    let task = tmp.path().join("task");
    std::fs::create_dir_all(&task).unwrap();
    std::fs::copy(example().join("README.md"), task.join("README.md")).unwrap();
    let logs = tmp.path().join("logs");
    let progress = tmp.path().join("progress.md");
    taskfmt::cmds::progress_init::generate_and_write(&example(), Some(&progress)).unwrap();
    std::fs::write(
        &progress,
        to_done(&std::fs::read_to_string(&progress).unwrap()),
    )
    .unwrap();
    let config = "schema = \"verify/v2\"\ntask_id = \"TASK-042\"\nbase_tree = \"792edca45d6c5c8570357d3864ad58d1f080d196\"\nwritable_paths = [\"*\"]\nforbidden_paths = [\"crates/pgtui/src/render.rs\"]\n\n[[checks]]\nid = \"CHK-001\"\nphase = \"gate\"\nargv = [\"true\"]\n";
    std::fs::write(task.join("verify.toml"), config).unwrap();

    let output = gate_at(&work, &task, &logs, &progress);
    assert!(
        !output
            .failed_checks
            .contains(&"forbidden_paths".to_string()),
        "untouched trusted file must not fail forbidden_paths:\n{}",
        output.text
    );

    // now the run edits the trusted file -> forbidden_paths must fail
    let output;
    {
        std::fs::write(work.join("crates/pgtui/src/render.rs"), "tampered\n").unwrap();
        output = gate_at(&work, &task, &logs, &progress);
    }
    assert!(
        output
            .failed_checks
            .contains(&"forbidden_paths".to_string()),
        "changed trusted file must fail forbidden_paths:\n{}",
        output.text
    );
    assert!(output.text.contains("CHANGED crates/pgtui/src/render.rs"));
}

#[test]
fn missing_config_is_exit_two_never_a_pass() {
    let tmp = tempfile::tempdir().unwrap();
    let (work, task) = fixture(tmp.path());
    std::fs::remove_file(task.join("verify.toml")).unwrap();
    let output = gate_at(
        &work,
        &task,
        &tmp.path().join("logs"),
        &tmp.path().join("progress.md"),
    );
    assert_eq!(output.exit, taskfmt::gate::EXIT_CONFIG);
    assert!(!output.is_pass());
    assert_eq!(output.last_line, "RESULT FAIL");
}

#[test]
fn checklist_normalization_is_the_only_allowed_drift() {
    let readme = std::fs::read_to_string(example().join("README.md")).unwrap();
    let done = readme.replace("- [ ]", "- [x]");
    assert_eq!(
        taskfmt::gate::checklist_normalized(&readme),
        taskfmt::gate::checklist_normalized(&done),
        "checking a box must not count as an edit"
    );
    assert_ne!(
        taskfmt::gate::checklist_normalized(&readme),
        taskfmt::gate::checklist_normalized(&done.replace("**3.1**", "**3.1** reworded")),
    );
    assert!(taskfile::checklist_block(&readme).len() >= 5);
}

// ---------------------------------------------------------------------------------------------
// Scope bypass matrix (D23): every way of hiding a change from `git diff` must still fail scope,
// while ordinary ignored files pass. Fresh workspace per case: no cross-case index state.
// ---------------------------------------------------------------------------------------------

const SCOPE_VERIFY_TOML: &str = r#"schema = "verify/v2"
task_id = "TASK-042"
base_tree = "792edca45d6c5c8570357d3864ad58d1f080d196"
writable_paths = ["src/allowed/*"]

[[checks]]
id = "CHK-001"
phase = "gate"
argv = ["true"]
"#;

fn git_out(dir: &Path, args: &[&str]) -> String {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir).args(args);
    let captured = ops::capture(&mut cmd).unwrap();
    assert!(captured.ok(), "git {:?}: {}", args, captured.stderr);
    captured.stdout
}

fn commit_all(dir: &Path, msg: &str) {
    git(dir, &["add", "-A"]);
    git(
        dir,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "-m",
            msg,
        ],
    );
}

/// Baseline: `src/allowed/a.rs`, `src/legacy/foo.rs`, committed `.gitignore` = `*.log`, tag
/// `baseline`. Returns (work, task, first-sha).
fn scope_fixture(dir: &Path) -> (PathBuf, PathBuf, String) {
    let work = dir.join("work");
    std::fs::create_dir_all(work.join("src/allowed")).unwrap();
    std::fs::create_dir_all(work.join("src/legacy")).unwrap();
    git(&work, &["init", "-q", "."]);
    std::fs::write(work.join("src/allowed/a.rs"), "a\n").unwrap();
    std::fs::write(work.join("src/legacy/foo.rs"), "foo\n").unwrap();
    std::fs::write(work.join(".gitignore"), "*.log\n").unwrap();
    commit_all(&work, "baseline");
    git(&work, &["tag", "baseline"]);
    let sha = git_out(&work, &["rev-parse", "HEAD"]).trim().to_string();
    let task = dir.join("task");
    std::fs::create_dir_all(&task).unwrap();
    std::fs::copy(example().join("README.md"), task.join("README.md")).unwrap();
    std::fs::write(task.join("verify.toml"), SCOPE_VERIFY_TOML).unwrap();
    (work, task, sha)
}

fn scope_gate(work: &Path, task: &Path, base: &str) -> gate::GateOutput {
    gate::run(GateOpts {
        root: work.to_path_buf(),
        task_dir: task.to_path_buf(),
        progress: None,
        base: Some(base.to_string()),
        log_dir: Some(work.parent().unwrap().join("logs")),
        fail_fast: false,
        enforce_task_contract: false,
    })
}

/// Run one scope scenario on a fresh fixture; `want_pass` is the whole-gate verdict.
fn scope_case(name: &str, want_pass: bool, setup: impl FnOnce(&Path)) -> gate::GateOutput {
    let tmp = tempfile::tempdir().unwrap();
    let (work, task, _) = scope_fixture(tmp.path());
    setup(&work);
    let output = scope_gate(&work, &task, "baseline");
    if want_pass {
        assert!(output.is_pass(), "{name}: must pass\n{}", output.text);
    } else {
        assert!(!output.is_pass(), "{name}: must fail\n{}", output.text);
        assert_eq!(
            output.failed_checks,
            vec!["scope"],
            "{name}: scope must be the failing check\n{}",
            output.text
        );
    }
    output
}

fn append(path: &Path, text: &str) {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().append(true).open(path).unwrap();
    f.write_all(text.as_bytes()).unwrap();
}

#[test]
fn scope_bypasses_fail_out_of_scope_tracked_edit() {
    let out = scope_case("out-of-scope tracked edit", false, |w| {
        append(&w.join("src/legacy/foo.rs"), "mod\n");
    });
    assert!(
        out.text.contains("OUTSIDE src/legacy/foo.rs"),
        "{}",
        out.text
    );
}

#[test]
fn scope_bypasses_fail_in_scope_edit_passes() {
    scope_case("in-scope edit", true, |w| {
        append(&w.join("src/allowed/a.rs"), "mod\n");
    });
}

#[test]
fn scope_bypasses_fail_ignored_log_passes() {
    scope_case("ordinary .gitignore'd file", true, |w| {
        std::fs::write(w.join("src/legacy/build.log"), "log\n").unwrap();
        std::fs::write(w.join("build.log"), "log\n").unwrap();
    });
}

#[test]
fn scope_bypasses_fail_out_of_scope_untracked_file() {
    let out = scope_case("out-of-scope untracked file", false, |w| {
        std::fs::write(w.join("src/legacy/new.rs"), "new\n").unwrap();
    });
    assert!(
        out.text.contains("OUTSIDE src/legacy/new.rs"),
        "{}",
        out.text
    );
}

#[test]
fn scope_bypasses_fail_staged_rename_out_to_in() {
    let out = scope_case("staged rename out->in", false, |w| {
        git(w, &["mv", "src/legacy/foo.rs", "src/allowed/foo.rs"]);
    });
    assert!(
        out.text.contains("OUTSIDE src/legacy/foo.rs"),
        "the deleted out-of-scope path must surface (--no-renames):\n{}",
        out.text
    );
}

#[test]
fn scope_bypasses_fail_info_exclude_hidden_file() {
    let out = scope_case(".git/info/exclude-hidden file", false, |w| {
        std::fs::create_dir_all(w.join(".git/info")).unwrap();
        append(&w.join(".git/info/exclude"), "hidden.rs\n");
        std::fs::write(w.join("src/legacy/hidden.rs"), "h\n").unwrap();
    });
    assert!(
        out.text.contains("OUTSIDE src/legacy/hidden.rs"),
        "{}",
        out.text
    );
}

#[test]
fn scope_bypasses_fail_untracked_self_ignoring_gitignore() {
    let out = scope_case("untracked self-ignoring .gitignore", false, |w| {
        std::fs::create_dir_all(w.join("src/legacy/sub")).unwrap();
        std::fs::write(w.join("src/legacy/sub/.gitignore"), "*\n").unwrap();
        std::fs::write(w.join("src/legacy/sub/z.rs"), "z\n").unwrap();
    });
    assert!(
        out.text.contains("OUTSIDE src/legacy/sub/.gitignore"),
        "{}",
        out.text
    );
}

#[test]
fn scope_bypasses_fail_skip_worktree_edit() {
    let out = scope_case("skip-worktree file", false, |w| {
        git(w, &["update-index", "--skip-worktree", "src/legacy/foo.rs"]);
        append(&w.join("src/legacy/foo.rs"), "mod\n");
    });
    assert!(out.text.contains("HIDDEN index entries"), "{}", out.text);
    assert!(out.text.contains("S src/legacy/foo.rs"), "{}", out.text);
}

#[test]
fn scope_bypasses_fail_assume_unchanged_edit() {
    let out = scope_case("assume-unchanged file", false, |w| {
        git(
            w,
            &["update-index", "--assume-unchanged", "src/legacy/foo.rs"],
        );
        append(&w.join("src/legacy/foo.rs"), "mod\n");
    });
    assert!(out.text.contains("HIDDEN index entries"), "{}", out.text);
    assert!(out.text.contains("h src/legacy/foo.rs"), "{}", out.text);
}

/// A moved `baseline` tag hides a prior out-of-scope commit; pinning the recorded base SHA
/// (D25: the harness records it at dispatch) still catches it.
#[test]
fn scope_bypasses_fail_base_sha_pins_past_moved_tag() {
    let tmp = tempfile::tempdir().unwrap();
    let (work, task, first_sha) = scope_fixture(tmp.path());
    append(&work.join("src/legacy/foo.rs"), "mod\n");
    commit_all(&work, "out");
    git(&work, &["tag", "-f", "baseline", "HEAD"]);

    let pinned = scope_gate(&work, &task, &first_sha);
    assert!(!pinned.is_pass(), "pinned base must fail:\n{}", pinned.text);
    assert_eq!(pinned.failed_checks, vec!["scope"], "{}", pinned.text);
    assert!(
        pinned.text.contains("OUTSIDE src/legacy/foo.rs"),
        "{}",
        pinned.text
    );

    let head = scope_gate(&work, &task, "HEAD");
    assert!(
        head.is_pass(),
        "HEAD base hides the prior commit:\n{}",
        head.text
    );
}

// ---------------------------------------------------------------------------------------------
// Command semantics: `bash -eo pipefail` — an early failing statement and a failing pipe stage
// both fail the check.
// ---------------------------------------------------------------------------------------------

fn command_gate(dir: &Path, focused: &[&str]) -> gate::GateOutput {
    let (work, task, _) = scope_fixture(dir);
    let checks = focused
        .iter()
        .enumerate()
        .map(|(index, command)| {
            format!(
                "[[checks]]\nid = \"CHK-{:03}\"\nphase = \"focused\"\nshell = {command:?}\n",
                index + 1
            )
        })
        .collect::<String>();
    let toml = SCOPE_VERIFY_TOML.replace(
        "[[checks]]\nid = \"CHK-001\"\nphase = \"gate\"\nargv = [\"true\"]",
        &format!("{checks}[[checks]]\nid = \"CHK-999\"\nphase = \"gate\"\nargv = [\"true\"]"),
    );
    std::fs::write(task.join("verify.toml"), toml).unwrap();
    scope_gate(&work, &task, "baseline")
}

#[test]
fn commands_run_with_errexit_early_failure_fails_check() {
    let tmp = tempfile::tempdir().unwrap();
    let out = command_gate(tmp.path(), &["false; true"]);
    assert!(!out.is_pass(), "{}", out.text);
    assert_eq!(out.failed_checks, vec!["CHK-001"], "{}", out.text);
}

#[test]
fn commands_run_with_pipefail_passing_pipe_passes() {
    let tmp = tempfile::tempdir().unwrap();
    let out = command_gate(tmp.path(), &["true | true"]);
    assert!(out.is_pass(), "{}", out.text);
}

#[test]
fn commands_run_with_pipefail_failing_stage_fails_check() {
    let tmp = tempfile::tempdir().unwrap();
    let out = command_gate(tmp.path(), &["false | true"]);
    assert!(!out.is_pass(), "{}", out.text);
    assert_eq!(out.failed_checks, vec!["CHK-001"], "{}", out.text);
}

#[test]
fn forbidden_pattern_errors_fail_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let (work, task) = fixture(tmp.path());
    let config = VERIFY_TOML.replacen(
        "writable_paths = [\"*\"]",
        "writable_paths = [\"*\"]\nforbidden_patterns = [{ regex = \"[\" }]",
        1,
    );
    std::fs::write(task.join("verify.toml"), config).unwrap();
    let output = gate::run(GateOpts {
        root: work,
        task_dir: task,
        progress: None,
        base: Some("baseline".to_string()),
        log_dir: Some(tmp.path().join("logs")),
        fail_fast: false,
        enforce_task_contract: false,
    });
    assert_eq!(output.failed_checks, ["forbidden_patterns"]);
    assert!(output.text.contains("failed rc=2"), "{}", output.text);
}

#[test]
fn fail_fast_stops_all_later_groups() {
    let tmp = tempfile::tempdir().unwrap();
    let (work, task) = fixture(tmp.path());
    let canary = tmp.path().join("later-ran");
    let config = VERIFY_TOML.replace(
        "[[checks]]\nid = \"CHK-001\"\nphase = \"gate\"\nargv = [\"true\"]",
        &format!(
            "[[checks]]\nid = \"CHK-001\"\nphase = \"focused\"\nshell = \"false\"\n\n[[checks]]\nid = \"CHK-002\"\nphase = \"regression\"\nshell = \"touch {}\"\n\n[[checks]]\nid = \"CHK-003\"\nphase = \"gate\"\nargv = [\"true\"]",
            canary.display()
        ),
    );
    std::fs::write(task.join("verify.toml"), config).unwrap();
    let output = gate::run(GateOpts {
        root: work,
        task_dir: task,
        progress: None,
        base: Some("baseline".to_string()),
        log_dir: Some(tmp.path().join("logs")),
        fail_fast: true,
        enforce_task_contract: false,
    });
    assert_eq!(output.failed_checks, ["CHK-001"]);
    assert!(!canary.exists(), "later regression command ran");
    assert!(output.check("CHK-002").is_none());
}

#[cfg(unix)]
#[test]
fn forbidden_pattern_path_symlink_escape_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let (work, task) = fixture(tmp.path());
    std::os::unix::fs::symlink(tmp.path(), work.join("escape")).unwrap();
    let config = VERIFY_TOML.replacen(
        "writable_paths = [\"*\"]",
        "writable_paths = [\"*\"]\nforbidden_patterns = [{ regex = \"never-matches\", paths = [\"escape\"] }]",
        1,
    );
    std::fs::write(task.join("verify.toml"), config).unwrap();
    let output = gate::run(GateOpts {
        root: work,
        task_dir: task,
        progress: None,
        base: Some("baseline".to_string()),
        log_dir: Some(tmp.path().join("logs")),
        fail_fast: false,
        enforce_task_contract: false,
    });
    assert_eq!(output.failed_checks, ["forbidden_patterns"]);
    assert!(output.text.contains("resolves outside repository"));
}
