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

const VERIFY_TOML: &str = r#"schema = "verify/v1"
base_ref = "baseline"
allowed_globs = ["*"]

[focused]
commands = []

[regression]
commands = []

[lint]
commands = []
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
    let line = done_line("**2** Required behavior");
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
            .filter(|line| !line.contains("**3.2**"))
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

    let config = |globs: &str| {
        format!(
            "schema = \"verify/v1\"\nbase_ref = \"baseline\"\nallowed_globs = {globs}\n\n[focused]\ncommands = []\n\n[regression]\ncommands = []\n\n[lint]\ncommands = []\n"
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

    // an empty whitelist fails closed
    let output = gate_with(&config("[]"));
    assert!(
        output.failed_checks.contains(&"scope".to_string()),
        "{}",
        output.text
    );

    // an unresolvable base fails scope
    std::fs::write(
        task.join("verify.toml"),
        config("[\"*\"]").replace("base_ref = \"baseline\"", "base_ref = \"no-such-ref\""),
    )
    .unwrap();
    let output = gate::run(GateOpts {
        root: work.to_path_buf(),
        task_dir: task.to_path_buf(),
        progress: Some(progress.display().to_string()),
        // no --base here: base_ref = "no-such-ref" must be the one that resolves
        base: None,
        log_dir: Some(logs.to_path_buf()),
        fail_fast: false,
    });
    assert!(
        output.failed_checks.contains(&"scope".to_string()),
        "{}",
        output.text
    );

    // base precedence: --base beats TASKFMT_BASE beats base_ref beats "baseline"
    let cfg =
        taskfmt::verifycfg::VerifyConfig::parse("schema = \"verify/v1\"\nbase_ref = \"fromcfg\"\n")
            .unwrap();
    assert_eq!(gate::resolve_base(&Some("flag".into()), &cfg), "flag");
    assert_eq!(
        gate::resolve_base_from(&None, Some("fromenv"), &cfg),
        "fromenv"
    );
    assert_eq!(gate::resolve_base_from(&None, None, &cfg), "fromcfg");
    let none = taskfmt::verifycfg::VerifyConfig::parse("schema = \"verify/v1\"\n").unwrap();
    assert_eq!(gate::resolve_base_from(&None, Some(""), &none), "baseline");
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
    let config = "schema = \"verify/v1\"\nbase_ref = \"baseline\"\nforbidden_paths = [\"crates/pgtui/src/render.rs\"]\nallowed_globs = [\"*\"]\n\n[focused]\ncommands = []\n\n[regression]\ncommands = []\n\n[lint]\ncommands = []\n";
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
