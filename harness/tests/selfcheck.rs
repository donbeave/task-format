//! `taskfmt selfcheck` on a toolchain-free package: nop / polarity / oracle on a throwaway git
//! workspace, driven through the library API (no network, no cargo, no docker).

use std::path::{Path, PathBuf};
use std::process::Command;

use taskfmt::ops;
use taskfmt::selfcheck::{self, EXIT_NOINPUT, EXIT_NOVERDICT, Report, SelfcheckOpts};

const VERIFY_TOML: &str = r#"schema = "verify/v2"
task_id = "TASK-001"
base_tree = "0000000000000000000000000000000000000000"
writable_paths = ["done.txt"]

[[checks]]
id = "CHK-001"
phase = "focused"
shell = "test -f done.txt"

[[checks]]
id = "CHK-002"
phase = "regression"
shell = "test -f keep.txt"

[[checks]]
id = "CHK-003"
phase = "gate"
argv = ["true"]
"#;

/// The second focused command is green on the baseline: polarity must catch it.
const VERIFY_TOML_GREEN_FOCUSED: &str = r#"schema = "verify/v2"
task_id = "TASK-002"
base_tree = "0000000000000000000000000000000000000000"
writable_paths = ["done.txt"]

[[checks]]
id = "CHK-001"
phase = "focused"
shell = "test -f done.txt"

[[checks]]
id = "CHK-002"
phase = "focused"
shell = "test -f keep.txt"

[[checks]]
id = "CHK-003"
phase = "regression"
shell = "test -f keep.txt"

[[checks]]
id = "CHK-004"
phase = "gate"
argv = ["true"]
"#;

/// `[regression]` lists the task's own new test (D28): red on the baseline, green on the reference.
const VERIFY_TOML_OWN_REGRESSION: &str = r#"schema = "verify/v2"
task_id = "TASK-003"
base_tree = "0000000000000000000000000000000000000000"
writable_paths = ["done.txt"]

[[checks]]
id = "CHK-001"
phase = "focused"
shell = "test -f done.txt"

[[checks]]
id = "CHK-002"
phase = "regression"
shell = "test -f done.txt"

[[checks]]
id = "CHK-003"
phase = "gate"
argv = ["true"]
"#;

/// focused.2 is not on PATH (rc 127), focused.3 exists but is not executable (rc 126).
const VERIFY_TOML_NOT_RUNNABLE: &str = r#"schema = "verify/v2"
task_id = "TASK-004"
base_tree = "0000000000000000000000000000000000000000"
writable_paths = ["done.txt"]

[[checks]]
id = "CHK-001"
phase = "focused"
shell = "test -f done.txt"

[[checks]]
id = "CHK-002"
phase = "focused"
shell = "taskfmt-no-such-toolchain-xyz --version"

[[checks]]
id = "CHK-003"
phase = "focused"
shell = "./keep.txt"

[[checks]]
id = "CHK-004"
phase = "regression"
shell = "test -f keep.txt"

[[checks]]
id = "CHK-005"
phase = "gate"
argv = ["true"]
"#;

const REF_PATCH: &str = "diff --git a/done.txt b/done.txt\nnew file mode 100644\n--- /dev/null\n+++ b/done.txt\n@@ -0,0 +1 @@\n+done\n";

fn git(dir: &Path, args: &[&str]) {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir).args(args);
    let captured = ops::capture(&mut cmd).unwrap();
    assert!(captured.ok(), "git {:?}: {}", args, captured.stderr);
}

/// A git workspace at a `baseline` commit holding `keep.txt` (+ `done.txt` when `solved`).
fn workspace(dir: &Path, name: &str, solved: bool) -> PathBuf {
    let work = dir.join(name);
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("keep.txt"), "keep\n").unwrap();
    if solved {
        std::fs::write(work.join("done.txt"), "done\n").unwrap();
    }
    git(&work, &["init", "-q", "."]);
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
            "baseline",
        ],
    );
    git(&work, &["tag", "baseline"]);
    work
}

fn task(dir: &Path, name: &str, toml: &str) -> PathBuf {
    let task = dir.join(name);
    std::fs::create_dir_all(&task).unwrap();
    std::fs::write(task.join("verify.toml"), toml).unwrap();
    task
}

fn reference(dir: &Path, name: &str, solved: bool) -> PathBuf {
    let reference = dir.join(name);
    std::fs::create_dir_all(&reference).unwrap();
    std::fs::write(reference.join("keep.txt"), "keep\n").unwrap();
    if solved {
        std::fs::write(reference.join("done.txt"), "done\n").unwrap();
    }
    reference
}

struct Fixture {
    _tmp: tempfile::TempDir,
    task: PathBuf,
    task_green_focused: PathBuf,
    workspace: PathBuf,
    workspace_solved: PathBuf,
    ref_ok: PathBuf,
    ref_bad: PathBuf,
    ref_patch: PathBuf,
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let ref_patch = dir.join("ref.patch");
    std::fs::write(&ref_patch, REF_PATCH).unwrap();
    Fixture {
        task: task(dir, "task", VERIFY_TOML),
        task_green_focused: task(dir, "task-pol", VERIFY_TOML_GREEN_FOCUSED),
        workspace: workspace(dir, "work", false),
        workspace_solved: workspace(dir, "work-green", true),
        ref_ok: reference(dir, "ref-ok", true),
        ref_bad: reference(dir, "ref-bad", false),
        ref_patch,
        _tmp: tmp,
    }
}

fn selfcheck(task: &Path, workspace: &Path, reference: Option<&Path>) -> Report {
    selfcheck::run(SelfcheckOpts {
        task_dir: task.to_path_buf(),
        workspace: workspace.to_path_buf(),
        base: "baseline".to_string(),
        reference: reference.map(Path::to_path_buf),
        keep: false,
    })
    .unwrap()
}

fn has_line(report: &Report, want: &str) -> bool {
    report.render().lines().any(|line| line == want)
}

fn assert_line(report: &Report, want: &str) {
    assert!(
        has_line(report, want),
        "missing {want:?} in:\n{}",
        report.render()
    );
}

/// The caller's workspace is never touched: same status before and after.
fn assert_untouched(workspace: &Path) {
    assert!(ops::git::status_porcelain(workspace).unwrap().is_empty());
    assert!(!workspace.join("done.txt").exists());
}

#[test]
fn correct_package_passes_all_three_phases() {
    let f = fixture();
    let report = selfcheck(&f.task, &f.workspace, Some(&f.ref_ok));
    assert!(report.pass, "{}", report.render());
    assert_eq!(report.exit_code(), 0);
    assert!(report.nop.pass);
    assert!(report.polarity.pass);
    assert!(report.oracle.as_ref().unwrap().pass);
    assert_line(&report, "SELFCHECK nop PASS");
    assert_line(&report, "SELFCHECK polarity PASS");
    assert_line(&report, "SELFCHECK oracle PASS");
    assert_line(&report, "SELFCHECK RESULT PASS");
    assert_line(
        &report,
        "POLARITY CHK-001 FAIL-ON-BASELINE OK got=FAIL cmd=shell=\"test -f done.txt\"",
    );
    assert_line(
        &report,
        "POLARITY CHK-002 PASS-ON-BASELINE INFO got=PASS cmd=shell=\"test -f keep.txt\"",
    );
    assert_line(
        &report,
        "POLARITY CHK-002 PASS-ON-REFERENCE OK got=PASS cmd=shell=\"test -f keep.txt\"",
    );
    assert!(!report.noverdict);
    assert_line(
        &report,
        "POLARITY CHK-001 PASS-ON-REFERENCE OK got=PASS cmd=shell=\"test -f done.txt\"",
    );
    assert_line(&report, "ORACLE apply PASS changed=1");
    assert!(report.render().ends_with("SELFCHECK RESULT PASS\n"));
    assert_untouched(&f.workspace);
}

#[test]
fn patch_reference_passes() {
    let f = fixture();
    let report = selfcheck(&f.task, &f.workspace, Some(&f.ref_patch));
    assert!(report.pass, "{}", report.render());
    assert_line(&report, "SELFCHECK oracle PASS");
    assert_line(&report, "ORACLE apply PASS changed=1");
    assert_untouched(&f.workspace);
}

#[test]
fn focused_command_green_at_baseline_fails_polarity_only() {
    let f = fixture();
    let report = selfcheck(&f.task_green_focused, &f.workspace, Some(&f.ref_ok));
    assert!(!report.pass);
    assert!(
        !report.noverdict,
        "a green focused command is a real verdict"
    );
    assert_eq!(report.exit_code(), 1);
    assert!(report.nop.pass);
    assert!(!report.polarity.pass);
    assert_line(&report, "SELFCHECK nop PASS");
    assert_line(&report, "SELFCHECK polarity FAIL");
    assert_line(
        &report,
        "POLARITY CHK-002 FAIL-ON-BASELINE BAD got=PASS cmd=shell=\"test -f keep.txt\"",
    );
    assert_line(&report, "SELFCHECK RESULT FAIL");
}

/// D28: a regression list may hold the task's own new tests — red on the baseline is information,
/// not a polarity verdict; the oracle still demands green on the reference.
#[test]
fn regression_failing_at_baseline_is_info_not_a_polarity_verdict() {
    let f = fixture();
    let task = task(f._tmp.path(), "task-own-reg", VERIFY_TOML_OWN_REGRESSION);
    let report = selfcheck(&task, &f.workspace, Some(&f.ref_ok));
    assert!(report.pass, "{}", report.render());
    assert_eq!(report.exit_code(), 0);
    assert!(report.polarity.pass);
    assert_line(&report, "SELFCHECK polarity PASS");
    assert_line(
        &report,
        "POLARITY CHK-002 PASS-ON-BASELINE INFO got=FAIL cmd=shell=\"test -f done.txt\"",
    );
    assert_line(
        &report,
        "POLARITY CHK-002 PASS-ON-REFERENCE OK got=PASS cmd=shell=\"test -f done.txt\"",
    );
    assert_line(&report, "SELFCHECK RESULT PASS");
    assert!(!has_line(
        &report,
        "POLARITY CHK-002 PASS-ON-BASELINE BAD got=FAIL cmd=shell=\"test -f done.txt\""
    ));

    // and a regression that stays red on the reference is still an oracle failure
    let report = selfcheck(&task, &f.workspace, Some(&f.ref_bad));
    assert!(!report.pass);
    assert!(report.polarity.pass);
    assert!(!report.oracle.as_ref().unwrap().pass);
    assert_line(
        &report,
        "POLARITY CHK-002 PASS-ON-REFERENCE BAD got=FAIL cmd=shell=\"test -f done.txt\"",
    );
    assert_eq!(report.exit_code(), 1);
}

/// A focused command that cannot run (rc 126/127) is no evidence of RED: NOVERDICT, exit 69.
#[test]
fn focused_not_runnable_is_noverdict_with_exit_69() {
    let f = fixture();
    let task = task(f._tmp.path(), "task-noverdict", VERIFY_TOML_NOT_RUNNABLE);
    let report = selfcheck(&task, &f.workspace, None);
    assert!(!report.pass);
    assert!(report.noverdict);
    assert_eq!(report.exit_code(), EXIT_NOVERDICT);
    assert_eq!(EXIT_NOVERDICT, 69);
    assert!(report.nop.pass, "{}", report.render());
    assert!(!report.polarity.pass);
    assert_line(
        &report,
        "POLARITY CHK-001 FAIL-ON-BASELINE OK got=FAIL cmd=shell=\"test -f done.txt\"",
    );
    assert_line(
        &report,
        "POLARITY CHK-002 FAIL-ON-BASELINE NOVERDICT rc=127 cmd=shell=\"taskfmt-no-such-toolchain-xyz --version\" (command not runnable: toolchain missing?)",
    );
    assert_line(
        &report,
        "POLARITY CHK-003 FAIL-ON-BASELINE NOVERDICT rc=126 cmd=shell=\"./keep.txt\" (command not runnable: toolchain missing?)",
    );
    assert_line(&report, "SELFCHECK polarity FAIL");
    assert_line(&report, "SELFCHECK RESULT FAIL");
    assert!(!has_line(
        &report,
        "POLARITY CHK-002 FAIL-ON-BASELINE OK got=FAIL cmd=shell=\"taskfmt-no-such-toolchain-xyz --version\""
    ));
    assert_untouched(&f.workspace);
}

#[test]
fn reference_without_solution_fails_oracle() {
    let f = fixture();
    let report = selfcheck(&f.task, &f.workspace, Some(&f.ref_bad));
    assert!(!report.pass);
    assert!(report.nop.pass);
    assert!(report.polarity.pass);
    assert!(!report.oracle.as_ref().unwrap().pass);
    assert_line(&report, "SELFCHECK oracle FAIL");
    assert_line(
        &report,
        "POLARITY CHK-001 PASS-ON-REFERENCE BAD got=FAIL cmd=shell=\"test -f done.txt\"",
    );
    assert_line(&report, "SELFCHECK RESULT FAIL");
}

#[test]
fn already_solved_workspace_fails_nop() {
    let f = fixture();
    let report = selfcheck(&f.task, &f.workspace_solved, Some(&f.ref_ok));
    assert!(!report.pass);
    assert!(!report.nop.pass);
    assert_line(&report, "SELFCHECK nop FAIL");
    assert_line(
        &report,
        "POLARITY CHK-001 FAIL-ON-BASELINE BAD got=PASS cmd=shell=\"test -f done.txt\"",
    );
    assert_line(&report, "SELFCHECK RESULT FAIL");
}

#[test]
fn no_reference_skips_oracle_and_passes_on_nop_and_polarity() {
    let f = fixture();
    let report = selfcheck(&f.task, &f.workspace, None);
    assert!(report.pass, "{}", report.render());
    assert!(report.oracle.is_none());
    assert_line(&report, "SELFCHECK oracle SKIPPED (no reference)");
    assert_line(&report, "SELFCHECK RESULT PASS");
    assert!(!has_line(&report, "SELFCHECK oracle PASS"));
    assert_untouched(&f.workspace);
}

#[test]
fn empty_focused_list_is_bad_polarity() {
    let f = fixture();
    let toml = VERIFY_TOML.replace("phase = \"focused\"", "phase = \"precondition\"");
    let task = task(f._tmp.path(), "task-empty", &toml);
    let report = selfcheck(&task, &f.workspace, None);
    // nothing fails on the baseline any more: nop is FAIL too, but polarity names the cause
    assert!(!report.polarity.pass);
    assert_line(
        &report,
        "POLARITY focused none BAD (no focused check: nothing proves RED on baseline)",
    );
    assert_line(&report, "SELFCHECK polarity FAIL");
    assert_line(&report, "SELFCHECK RESULT FAIL");
}

#[test]
fn keep_retains_the_scratch_copy() {
    let f = fixture();
    let report = selfcheck::run(SelfcheckOpts {
        task_dir: f.task.clone(),
        workspace: f.workspace.clone(),
        base: "baseline".to_string(),
        reference: Some(f.ref_ok.clone()),
        keep: true,
    })
    .unwrap();
    let kept = report.kept.clone().expect("kept path");
    assert!(kept.join("work/done.txt").is_file(), "{}", kept.display());
    assert!(kept.join("logs/nop").is_dir());
    assert!(kept.join("logs/oracle").is_dir());
    assert_line(&report, &format!("SELFCHECK work kept {}", kept.display()));
    std::fs::remove_dir_all(kept).unwrap();
}

#[test]
fn missing_inputs_map_to_exit_66() {
    let f = fixture();
    let cases: Vec<(&str, PathBuf, PathBuf, Option<PathBuf>)> = vec![
        (
            "task dir",
            f._tmp.path().join("no-such-task"),
            f.workspace.clone(),
            None,
        ),
        (
            "verify.toml",
            {
                let empty = f._tmp.path().join("task-no-toml");
                std::fs::create_dir_all(&empty).unwrap();
                empty
            },
            f.workspace.clone(),
            None,
        ),
        (
            "workspace",
            f.task.clone(),
            f._tmp.path().join("no-such-work"),
            None,
        ),
        (
            "workspace not a repo",
            f.task.clone(),
            {
                let plain = f._tmp.path().join("plain");
                std::fs::create_dir_all(&plain).unwrap();
                plain
            },
            None,
        ),
        (
            "reference",
            f.task.clone(),
            f.workspace.clone(),
            Some(f._tmp.path().join("no-such-ref")),
        ),
    ];
    for (what, task_dir, workspace, reference) in cases {
        let err = selfcheck::run(SelfcheckOpts {
            task_dir,
            workspace,
            base: "baseline".to_string(),
            reference,
            keep: false,
        })
        .expect_err(what);
        assert_eq!(
            selfcheck::exit_code_for(&err),
            EXIT_NOINPUT,
            "{what}: {err:#}"
        );
    }
}

#[test]
fn unresolvable_base_is_a_nop_failure_not_an_internal_error() {
    let f = fixture();
    let report = selfcheck::run(SelfcheckOpts {
        task_dir: f.task.clone(),
        workspace: f.workspace.clone(),
        base: "no-such-ref".to_string(),
        reference: None,
        keep: false,
    })
    .unwrap();
    // the scope check fails on an unresolvable base: nop still sees a real gate FAIL (rc=1), the
    // focused command still fails => polarity holds; the package is not proven, only the base is
    assert!(report.nop.pass, "{}", report.render());
    assert!(
        report
            .render()
            .lines()
            .any(|line| line.starts_with("  | CHECK scope FAIL rc=1 log=")),
        "{}",
        report.render()
    );
}
