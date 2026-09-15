//! `taskfmt verify` — the gate command the agent runs inside the container and the operator runs on
//! the host. Exit 0 AND last stdout line exactly `DONE` <=> pass.

use std::path::PathBuf;

use taskfmt::gate::{self, GateOpts};
use taskfmt::ops::git;

#[allow(clippy::too_many_arguments)]
pub fn run(
    root: Option<&std::path::Path>,
    task_dir: Option<&std::path::Path>,
    progress: Option<String>,
    base: Option<String>,
    log_dir: Option<&std::path::Path>,
    fail_fast: bool,
) -> anyhow::Result<i32> {
    let opts = GateOpts {
        root: resolve_root(root)?,
        task_dir: resolve_task_dir(task_dir)?,
        progress: Some(resolve_progress(progress)),
        base,
        log_dir: log_dir.map(PathBuf::from),
        fail_fast,
        enforce_task_contract: true,
    };
    let output = gate::run(opts);
    print!("{}", output.text);
    let _ = std::io::Write::flush(&mut std::io::stdout());
    Ok(output.exit)
}

/// `--root` / `$TASKFMT_ROOT` (via clap) > git toplevel of cwd > cwd.
pub fn resolve_root(explicit: Option<&std::path::Path>) -> anyhow::Result<PathBuf> {
    if let Some(root) = explicit {
        return Ok(root.to_path_buf());
    }
    let cwd = std::env::current_dir()?;
    if let Some(toplevel) = git::toplevel(&cwd) {
        return Ok(toplevel);
    }
    Ok(cwd)
}

/// `--task-dir` / `$TASKFMT_TASK_DIR` (via clap) > `/task` (container layout) > cwd.
pub fn resolve_task_dir(explicit: Option<&std::path::Path>) -> anyhow::Result<PathBuf> {
    if let Some(dir) = explicit {
        return Ok(dir.to_path_buf());
    }
    let task_dir = PathBuf::from("/task");
    if task_dir.join("README.md").is_file() || task_dir.join("verify.toml").is_file() {
        return Ok(task_dir);
    }
    Ok(std::env::current_dir()?)
}

/// `--progress` / `$PROGRESS_FILE` (via clap) > `/progress/progress.md`. Empty string disables the check.
pub fn resolve_progress(explicit: Option<String>) -> String {
    explicit.unwrap_or_else(|| "/progress/progress.md".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_flags_win() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(resolve_root(Some(dir.path())).unwrap(), dir.path());
        assert_eq!(resolve_task_dir(Some(dir.path())).unwrap(), dir.path());
        assert_eq!(resolve_progress(Some(String::new())), "");
        assert_eq!(
            resolve_progress(Some("/p/progress.md".into())),
            "/p/progress.md"
        );
    }

    #[test]
    fn env_progress_beats_the_container_default() {
        use crate::cli::Cli;
        use clap::Parser as _;
        assert_eq!(resolve_progress(None), "/progress/progress.md");
        assert_eq!(resolve_progress(Some(String::new())), "");
        let cli =
            Cli::try_parse_from(["taskfmt", "verify", "--progress", "/run/progress.md"]).unwrap();
        let crate::cli::Command::Verify(opts) = cli.command else {
            panic!("expected verify");
        };
        assert_eq!(resolve_progress(opts.progress), "/run/progress.md");
    }

    #[test]
    fn redact_gate_output_never_leaks() {
        taskfmt::redact::register("super-secret-token-value");
        let text = "CHECK focused.1 FAIL rc=1 super-secret-token-value\n";
        assert!(!taskfmt::redact::scrub(text).contains("super-secret-token-value"));
    }
}
