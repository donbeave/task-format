//! `taskfmt selfcheck` — prove one task package's gate distinguishes "not done" from "done" (D13).
//!
//! Precedent: SWE-bench FAIL_TO_PASS / PASS_TO_PASS, Harbor Oracle / Nop solvers.
//!
//! - **nop**: the gate on the untouched workspace (a git repo at the trusted base commit) must exit
//!   1 with `RESULT FAIL` from real checks — exit 2 (no config) and 70 (internal error) are not a
//!   gate verdict.
//! - **polarity**: from the very same nop run (no second execution): every `focused.N` must FAIL
//!   on the baseline (delta). An empty `focused` list proves nothing RED and is BAD. A focused
//!   command that is not runnable (rc 126/127, or the shell could not be spawned) is **NOVERDICT**:
//!   its failure says nothing about the package (toolchain missing?), the phase FAILs and the
//!   report exits 69 (`EX_UNAVAILABLE`) — never a vacuous RED. `regression.N` results are reported
//!   as `INFO` only and never affect the verdict: D28 lets a package list its own new tests under
//!   `[regression]`, so those legitimately fail on the baseline.
//! - **oracle** (only with a reference): the reference applied over a scratch copy of the workspace
//!   (dir: mirror with delete semantics, `.git` kept; `.patch`/`.diff`: `git apply`), everything
//!   staged, the gate must PASS with `DONE`, and every focused/regression check must PASS.
//!
//! The progress check is disabled for every run: it is the executor's business, not the gate's
//! polarity. The caller's workspace is never mutated — all phases run in a scratch copy under
//! `TMPDIR` (`keep` retains it).

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;

use crate::gate::{self, GateOpts, GateOutput};
use crate::ops;
use crate::verifycfg::{FILE_NAME, VerifyConfig};

/// Exit code of a fully passing selfcheck.
pub const EXIT_PASS: i32 = 0;
/// Exit code when any phase failed.
pub const EXIT_FAIL: i32 = 1;
/// Exit code for a usage error (bad arguments).
pub const EXIT_USAGE: i32 = 64;
/// Exit code for a missing input (task dir, verify.toml, workspace, reference).
pub const EXIT_NOINPUT: i32 = 66;
/// Exit code when a focused command was not runnable on the baseline (rc 126/127): no verdict,
/// `EX_UNAVAILABLE`.
pub const EXIT_NOVERDICT: i32 = 69;
/// Exit code for an internal error.
pub const EXIT_INTERNAL: i32 = 70;

#[derive(Debug, Clone)]
pub struct SelfcheckOpts {
    /// Directory holding `verify.toml` (+ `README.md`): the trusted copies on the host.
    pub task_dir: PathBuf,
    /// A git repository checked out at the trusted base commit; never mutated.
    pub workspace: PathBuf,
    /// Scope base: a sha or ref resolvable inside `workspace` (e.g. `baseline`).
    pub base: String,
    /// Reference solution: a directory mirrored over the tree, or a `.patch`/`.diff` file.
    pub reference: Option<PathBuf>,
    /// Retain the scratch copy (path reported in the output).
    pub keep: bool,
}

/// One phase: its verdict and the lines it printed (verbatim gate output indented `  | `).
#[derive(Debug, Clone)]
pub struct Phase {
    pub pass: bool,
    pub lines: Vec<String>,
}

impl Phase {
    fn new(pass: bool, lines: Vec<String>) -> Self {
        Self { pass, lines }
    }
}

#[derive(Debug, Clone)]
pub struct Report {
    /// Header lines (`SELFCHECK task …`, `SELFCHECK work …`).
    pub header: Vec<String>,
    pub nop: Phase,
    pub polarity: Phase,
    /// `None` when no reference was given (oracle SKIPPED).
    pub oracle: Option<Phase>,
    /// Overall verdict: nop AND polarity AND (oracle, when it ran).
    pub pass: bool,
    /// A focused command was not runnable on the baseline (rc 126/127): the FAIL is no verdict.
    pub noverdict: bool,
    /// The retained scratch copy (`keep`), if any.
    pub kept: Option<PathBuf>,
}

impl Report {
    /// 0 PASS, 69 when any focused command was NOVERDICT (takes precedence: the FAIL is not a
    /// verdict on the package), 1 FAIL otherwise.
    pub fn exit_code(&self) -> i32 {
        if self.pass {
            EXIT_PASS
        } else if self.noverdict {
            EXIT_NOVERDICT
        } else {
            EXIT_FAIL
        }
    }

    /// Full stdout of the command, ending with `SELFCHECK RESULT PASS|FAIL`.
    pub fn render(&self) -> String {
        let mut out: Vec<String> = self.header.clone();
        out.extend(self.nop.lines.iter().cloned());
        out.push(format!("SELFCHECK nop {}", verdict(self.nop.pass)));
        out.extend(self.polarity.lines.iter().cloned());
        out.push(format!(
            "SELFCHECK polarity {}",
            verdict(self.polarity.pass)
        ));
        match &self.oracle {
            Some(oracle) => {
                out.extend(oracle.lines.iter().cloned());
                out.push(format!("SELFCHECK oracle {}", verdict(oracle.pass)));
            }
            None => out.push("SELFCHECK oracle SKIPPED (no reference)".to_string()),
        }
        if let Some(kept) = &self.kept {
            out.push(format!("SELFCHECK work kept {}", kept.display()));
        }
        out.push(format!("SELFCHECK RESULT {}", verdict(self.pass)));
        crate::redact::scrub(&(out.join("\n") + "\n"))
    }
}

fn verdict(pass: bool) -> &'static str {
    if pass { "PASS" } else { "FAIL" }
}

/// A required input is absent: maps to exit 66 (the caller downcasts via [`exit_code_for`]).
#[derive(Debug)]
pub struct MissingInput(pub String);

impl fmt::Display for MissingInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MissingInput {}

/// Exit code for an error returned by [`run`]: 66 for a missing input, 70 otherwise.
pub fn exit_code_for(err: &anyhow::Error) -> i32 {
    if err.downcast_ref::<MissingInput>().is_some() {
        EXIT_NOINPUT
    } else {
        EXIT_INTERNAL
    }
}

fn missing(what: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(MissingInput(what.into()))
}

/// Run the three phases. `Err` only for missing inputs or internal failures — a failing phase is
/// a normal `Report` with `pass == false`.
pub fn run(opts: SelfcheckOpts) -> anyhow::Result<Report> {
    // ---------- inputs ----------
    if !opts.task_dir.is_dir() {
        return Err(missing(format!(
            "no such task dir: {}",
            opts.task_dir.display()
        )));
    }
    let verify_config = opts.task_dir.join(FILE_NAME);
    if !verify_config.is_file() {
        return Err(missing(format!("missing {}", verify_config.display())));
    }
    if !opts.workspace.is_dir() {
        return Err(missing(format!(
            "no such workspace: {}",
            opts.workspace.display()
        )));
    }
    if !ops::git::is_repo(&opts.workspace) {
        return Err(missing(format!(
            "workspace is not a git repository: {}",
            opts.workspace.display()
        )));
    }
    let reference = match &opts.reference {
        Some(path) if !path.exists() => {
            return Err(missing(format!("no such reference: {}", path.display())));
        }
        Some(path) => Some(std::path::absolute(path)?),
        None => None,
    };
    let cfg = VerifyConfig::load(&verify_config)
        .with_context(|| format!("loading {}", verify_config.display()))?;

    // ---------- scratch copy: the caller's tree is never touched ----------
    let scratch = tempfile::Builder::new()
        .prefix("taskfmt-selfcheck.")
        .tempdir()
        .context("creating the scratch dir under TMPDIR")?;
    let work = scratch.path().join("work");
    let logs = scratch.path().join("logs");
    std::fs::create_dir_all(&work)?;
    std::fs::create_dir_all(&logs)?;
    ops::copy_tree(&opts.workspace, &work)
        .with_context(|| format!("copying {} to {}", opts.workspace.display(), work.display()))?;

    let mut header = vec![
        format!("SELFCHECK task {}", opts.task_dir.display()),
        format!("SELFCHECK workspace {}", opts.workspace.display()),
        format!("SELFCHECK base {}", opts.base),
    ];
    match &reference {
        Some(path) => header.push(format!("SELFCHECK reference {}", path.display())),
        None => header.push("SELFCHECK reference (none)".to_string()),
    }
    header.push(format!("SELFCHECK work {}", work.display()));

    let gate_at = |phase: &str| -> GateOutput {
        gate::run(GateOpts {
            root: work.clone(),
            task_dir: opts.task_dir.clone(),
            progress: None,
            base: Some(opts.base.clone()),
            log_dir: Some(logs.join(phase)),
            fail_fast: false,
            enforce_task_contract: false,
        })
    };

    // ---------- 1. nop ----------
    let nop_gate = gate_at("nop");
    let nop = nop_phase(&nop_gate);

    // ---------- 2. polarity (same run, no second execution) ----------
    let polarity = polarity_phase(&cfg, &nop_gate);

    // ---------- 3. oracle ----------
    let oracle = reference
        .as_deref()
        .map(|reference| oracle_phase(&cfg, &work, reference, &opts.base, &gate_at));

    let pass = nop.pass && polarity.phase.pass && oracle.as_ref().is_none_or(|phase| phase.pass);
    let noverdict = polarity.noverdict;
    let kept = if opts.keep {
        Some(scratch.keep())
    } else {
        None
    };
    Ok(Report {
        header,
        nop,
        polarity: polarity.phase,
        oracle,
        pass,
        noverdict,
        kept,
    })
}

fn indented(text: &str) -> impl Iterator<Item = String> + '_ {
    text.lines().map(|line| format!("  | {line}"))
}

fn nop_phase(gate: &GateOutput) -> Phase {
    let mut lines = vec!["SELFCHECK phase nop".to_string()];
    lines.extend(indented(&gate.text));
    let failing = gate.failed_checks.join(",");
    let pass = gate.exit == gate::EXIT_FAIL && !gate.failed_checks.is_empty();
    if pass {
        lines.push(format!("NOP rc={} failing={failing}", gate.exit));
    } else {
        lines.push(format!(
            "NOP rc={} failing={failing} (want rc=1 with at least one CHECK FAIL; rc=2 no config, rc=70 internal error)",
            gate.exit
        ));
    }
    Phase::new(pass, lines)
}

/// Return codes that mean "the command never ran", not "the command failed": 126 (found, not
/// executable) and 127 (not found; also the gate's code for a shell that could not be spawned).
fn not_runnable(rc: i32) -> bool {
    rc == 126 || rc == 127
}

fn got_text(got: Option<&gate::CheckResult>) -> &'static str {
    match got {
        Some(check) => verdict(check.pass),
        None => "MISSING",
    }
}

/// Emit one `POLARITY` line per command of `prefix`; `false` when any verdict is BAD.
fn polarity_lines(
    lines: &mut Vec<String>,
    gate: &GateOutput,
    prefix: &str,
    commands: &[String],
    want_pass: bool,
    label: &str,
) -> bool {
    let want = verdict(want_pass);
    let mut ok = true;
    for (i, cmd) in commands.iter().enumerate() {
        let name = format!("{prefix}.{}", i + 1);
        let got = gate.check_result(&name);
        let good = got.is_some_and(|check| check.pass == want_pass);
        ok &= good;
        lines.push(format!(
            "POLARITY {name} {want}-ON-{label} {} got={} cmd={cmd}",
            if good { "OK" } else { "BAD" },
            got_text(got)
        ));
    }
    ok
}

/// The polarity phase plus whether any focused command was NOVERDICT.
struct PolarityOutcome {
    phase: Phase,
    noverdict: bool,
}

/// Every `focused.N` must FAIL on the baseline for a real reason: a not-runnable command (rc
/// 126/127) is NOVERDICT — BAD for the phase, but flagged apart so the caller can tell "toolchain
/// missing" from "the package is wrong".
fn focused_polarity_lines(
    lines: &mut Vec<String>,
    gate: &GateOutput,
    commands: &[String],
) -> (bool, bool) {
    let mut ok = true;
    let mut noverdict = false;
    for (i, cmd) in commands.iter().enumerate() {
        let name = format!("focused.{}", i + 1);
        let got = gate.check_result(&name);
        match got {
            Some(check) if !check.pass && not_runnable(check.rc) => {
                ok = false;
                noverdict = true;
                lines.push(format!(
                    "POLARITY {name} FAIL-ON-BASELINE NOVERDICT rc={} cmd={cmd} (command not runnable: toolchain missing?)",
                    check.rc
                ));
            }
            _ => {
                let good = got.is_some_and(|check| !check.pass);
                ok &= good;
                lines.push(format!(
                    "POLARITY {name} FAIL-ON-BASELINE {} got={} cmd={cmd}",
                    if good { "OK" } else { "BAD" },
                    got_text(got)
                ));
            }
        }
    }
    (ok, noverdict)
}

/// `regression.N` on the baseline is information only (D28: the list may hold the task's own new
/// tests): one `INFO` line each, never a verdict.
fn regression_info_lines(lines: &mut Vec<String>, gate: &GateOutput, commands: &[String]) {
    for (i, cmd) in commands.iter().enumerate() {
        let name = format!("regression.{}", i + 1);
        lines.push(format!(
            "POLARITY {name} PASS-ON-BASELINE INFO got={} cmd={cmd}",
            got_text(gate.check_result(&name))
        ));
    }
}

fn polarity_phase(cfg: &VerifyConfig, nop: &GateOutput) -> PolarityOutcome {
    let mut lines = vec!["SELFCHECK phase polarity".to_string()];
    let mut pass = true;
    if cfg.focused.commands.is_empty() {
        pass = false;
        lines.push(
            "POLARITY focused none BAD ([focused] commands empty: nothing proves RED on baseline)"
                .to_string(),
        );
    }
    let (focused_ok, noverdict) = focused_polarity_lines(&mut lines, nop, &cfg.focused.commands);
    pass &= focused_ok;
    regression_info_lines(&mut lines, nop, &cfg.regression.commands);
    PolarityOutcome {
        phase: Phase::new(pass, lines),
        noverdict,
    }
}

fn oracle_phase(
    cfg: &VerifyConfig,
    work: &Path,
    reference: &Path,
    base: &str,
    gate_at: &dyn Fn(&str) -> GateOutput,
) -> Phase {
    let mut lines = vec!["SELFCHECK phase oracle".to_string()];
    if let Err(err) = apply_reference(work, reference) {
        lines.push("ORACLE apply FAIL".to_string());
        lines.extend(indented(&format!("{err:#}")));
        return Phase::new(false, lines);
    }
    if let Err(err) = ops::git::add_all(work) {
        lines.push("ORACLE apply FAIL (git add -A)".to_string());
        lines.extend(indented(&format!("{err:#}")));
        return Phase::new(false, lines);
    }
    let changed = staged_count(work, base);
    lines.push(format!("ORACLE apply PASS changed={changed}"));

    let gate = gate_at("oracle");
    lines.extend(indented(&gate.text));
    let mut pass = true;
    if gate.is_pass() {
        lines.push("ORACLE rc=0 last=DONE".to_string());
    } else {
        pass = false;
        lines.push(format!(
            "ORACLE rc={} last={} failing={} (want rc=0, last line DONE)",
            gate.exit,
            gate.last_line,
            gate.failed_checks.join(",")
        ));
    }
    pass &= polarity_lines(
        &mut lines,
        &gate,
        "focused",
        &cfg.focused.commands,
        true,
        "REFERENCE",
    );
    pass &= polarity_lines(
        &mut lines,
        &gate,
        "regression",
        &cfg.regression.commands,
        true,
        "REFERENCE",
    );
    Phase::new(pass, lines)
}

fn staged_count(work: &Path, base: &str) -> usize {
    let mut cmd = Command::new("git");
    cmd.current_dir(work)
        .args(["diff", "--cached", "--name-only", base, "--"]);
    ops::git::output(&mut cmd)
        .map(|out| out.lines().filter(|line| !line.is_empty()).count())
        .unwrap_or(0)
}

/// Apply the reference over `work`: a directory is mirrored (rsync `-a --delete --exclude .git`
/// semantics), a `.patch`/`.diff` goes through `git apply --whitespace=nowarn`.
pub fn apply_reference(work: &Path, reference: &Path) -> anyhow::Result<()> {
    if reference.is_dir() {
        return mirror_dir(reference, work);
    }
    let ext = reference
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    if reference.is_file() && (ext == "patch" || ext == "diff") {
        let mut cmd = Command::new("git");
        cmd.current_dir(work)
            .arg("apply")
            .arg("--whitespace=nowarn")
            .arg(reference);
        let captured = ops::capture(&mut cmd)?;
        if !captured.ok() {
            anyhow::bail!(
                "git apply {} failed (rc={}): {}",
                reference.display(),
                captured.status,
                crate::redact::scrub(captured.stderr.trim_end())
            );
        }
        return Ok(());
    }
    anyhow::bail!(
        "reference must be a directory or a .patch/.diff file: {}",
        reference.display()
    )
}

/// `rsync -a --delete --exclude .git src/ dst/`: every entry of `dst` absent from `src` is removed
/// (`.git` untouched), then `src` is copied over `dst`.
fn mirror_dir(src: &Path, dst: &Path) -> anyhow::Result<()> {
    let git = Path::new(".git");
    let mut doomed: Vec<(PathBuf, bool)> = Vec::new();
    for entry in walkdir::WalkDir::new(dst)
        .follow_links(false)
        .contents_first(true)
    {
        let entry = entry?;
        let rel = entry.path().strip_prefix(dst)?.to_path_buf();
        if rel.as_os_str().is_empty() || rel.starts_with(git) {
            continue;
        }
        if std::fs::symlink_metadata(src.join(&rel)).is_err() {
            doomed.push((entry.path().to_path_buf(), entry.file_type().is_dir()));
        }
    }
    for (path, is_dir) in doomed {
        if is_dir {
            std::fs::remove_dir_all(&path)
                .with_context(|| format!("removing {}", path.display()))?;
        } else {
            std::fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
        }
    }
    ops::copy_tree_filtered(src, dst, &|rel| rel != git)
        .with_context(|| format!("copying {} over {}", src.display(), dst.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_deletes_extras_and_copies_new_files_but_keeps_git() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::create_dir_all(dst.join(".git")).unwrap();
        std::fs::create_dir_all(dst.join("gone")).unwrap();
        std::fs::write(src.join("keep.txt"), "new\n").unwrap();
        std::fs::write(src.join("sub/a.txt"), "a\n").unwrap();
        std::fs::write(dst.join("keep.txt"), "old\n").unwrap();
        std::fs::write(dst.join("extra.txt"), "x\n").unwrap();
        std::fs::write(dst.join("gone/b.txt"), "b\n").unwrap();
        std::fs::write(dst.join(".git/HEAD"), "ref\n").unwrap();
        mirror_dir(&src, &dst).unwrap();
        assert_eq!(
            std::fs::read_to_string(dst.join("keep.txt")).unwrap(),
            "new\n"
        );
        assert_eq!(
            std::fs::read_to_string(dst.join("sub/a.txt")).unwrap(),
            "a\n"
        );
        assert!(!dst.join("extra.txt").exists());
        assert!(!dst.join("gone").exists());
        assert!(dst.join(".git/HEAD").is_file());
    }

    #[test]
    fn exit_codes_distinguish_missing_input_from_internal() {
        assert_eq!(exit_code_for(&missing("x")), EXIT_NOINPUT);
        assert_eq!(exit_code_for(&anyhow::anyhow!("boom")), EXIT_INTERNAL);
        assert_eq!(
            exit_code_for(&missing("x").context("wrapped")),
            EXIT_NOINPUT
        );
    }

    #[test]
    fn unknown_reference_kind_is_an_oracle_apply_error() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("ref.tar");
        std::fs::write(&file, "x").unwrap();
        let err = apply_reference(tmp.path(), &file).unwrap_err();
        assert!(err.to_string().contains("directory or a .patch/.diff"));
    }

    #[test]
    fn verifycfg_focused_is_the_polarity_source() {
        let cfg =
            VerifyConfig::parse("schema = \"verify/v1\"\n[focused]\ncommands = []\n").unwrap();
        let empty = GateOutput {
            exit: gate::EXIT_FAIL,
            text: String::new(),
            last_line: "RESULT FAIL".into(),
            summary: String::new(),
            log_dir: PathBuf::new(),
            failed_checks: vec!["scope".into()],
            checks: vec![],
        };
        let outcome = polarity_phase(&cfg, &empty);
        assert!(!outcome.phase.pass);
        assert!(!outcome.noverdict);
        assert!(
            outcome
                .phase
                .lines
                .iter()
                .any(|l| l.starts_with("POLARITY focused none BAD"))
        );
    }

    fn gate_with(checks: Vec<gate::CheckResult>) -> GateOutput {
        let failed_checks = checks
            .iter()
            .filter(|check| !check.pass)
            .map(|check| check.name.clone())
            .collect();
        GateOutput {
            exit: gate::EXIT_FAIL,
            text: String::new(),
            last_line: "RESULT FAIL".into(),
            summary: String::new(),
            log_dir: PathBuf::new(),
            failed_checks,
            checks,
        }
    }

    fn check(name: &str, rc: i32) -> gate::CheckResult {
        gate::CheckResult {
            name: name.into(),
            pass: rc == 0,
            rc,
        }
    }

    #[test]
    fn focused_rc_126_or_127_is_noverdict_and_regression_is_info_only() {
        let cfg = VerifyConfig::parse(
            "schema = \"verify/v1\"\n[focused]\ncommands = [\"a\", \"b\", \"c\"]\n[regression]\ncommands = [\"r\"]\n",
        )
        .unwrap();
        let outcome = polarity_phase(
            &cfg,
            &gate_with(vec![
                check("focused.1", 1),
                check("focused.2", 127),
                check("focused.3", 126),
                check("regression.1", 1),
            ]),
        );
        assert!(!outcome.phase.pass);
        assert!(outcome.noverdict);
        let lines = outcome.phase.lines.join("\n");
        assert!(
            lines.contains("POLARITY focused.1 FAIL-ON-BASELINE OK got=FAIL cmd=a"),
            "{lines}"
        );
        assert!(
            lines.contains("POLARITY focused.2 FAIL-ON-BASELINE NOVERDICT rc=127 cmd=b (command not runnable: toolchain missing?)"),
            "{lines}"
        );
        assert!(
            lines.contains("POLARITY focused.3 FAIL-ON-BASELINE NOVERDICT rc=126 cmd=c"),
            "{lines}"
        );
        assert!(
            lines.contains("POLARITY regression.1 PASS-ON-BASELINE INFO got=FAIL cmd=r"),
            "{lines}"
        );

        // a failing regression alone never fails the phase
        let outcome = polarity_phase(
            &cfg,
            &gate_with(vec![
                check("focused.1", 1),
                check("focused.2", 1),
                check("focused.3", 2),
                check("regression.1", 1),
            ]),
        );
        assert!(outcome.phase.pass, "{}", outcome.phase.lines.join("\n"));
        assert!(!outcome.noverdict);
    }

    #[test]
    fn exit_code_prefers_noverdict_over_plain_fail() {
        let phase = |pass: bool| Phase::new(pass, vec![]);
        let mut report = Report {
            header: vec![],
            nop: phase(true),
            polarity: phase(false),
            oracle: None,
            pass: false,
            noverdict: true,
            kept: None,
        };
        assert_eq!(report.exit_code(), EXIT_NOVERDICT);
        report.noverdict = false;
        assert_eq!(report.exit_code(), EXIT_FAIL);
        report.pass = true;
        report.polarity = phase(true);
        assert_eq!(report.exit_code(), EXIT_PASS);
    }
}
