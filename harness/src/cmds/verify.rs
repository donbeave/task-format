//! `taskfmt verify` — the gate command the agent runs inside the container and the operator runs on
//! the host. Exit 0 AND last stdout line exactly `DONE` <=> pass.

use std::path::PathBuf;

use crate::gate::{self, GateOpts};
use crate::ops::git;

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
    };
    let output = gate::run(opts);
    print!("{}", output.text);
    let _ = std::io::Write::flush(&mut std::io::stdout());
    Ok(output.exit)
}

/// `--root` > `TASKFMT_ROOT` > git toplevel of cwd > cwd (the verify.sh default order).
pub fn resolve_root(explicit: Option<&std::path::Path>) -> anyhow::Result<PathBuf> {
    if let Some(root) = explicit {
        return Ok(root.to_path_buf());
    }
    if let Ok(env_root) = std::env::var("TASKFMT_ROOT")
        && !env_root.is_empty()
    {
        return Ok(PathBuf::from(env_root));
    }
    let cwd = std::env::current_dir()?;
    if let Some(toplevel) = git::toplevel(&cwd) {
        return Ok(toplevel);
    }
    Ok(cwd)
}

/// `--task-dir` > `TASKFMT_TASK_DIR` > `/task` (container layout) > cwd.
pub fn resolve_task_dir(explicit: Option<&std::path::Path>) -> anyhow::Result<PathBuf> {
    if let Some(dir) = explicit {
        return Ok(dir.to_path_buf());
    }
    if let Ok(env_dir) = std::env::var("TASKFMT_TASK_DIR")
        && !env_dir.is_empty()
    {
        return Ok(PathBuf::from(env_dir));
    }
    let task_dir = PathBuf::from("/task");
    if task_dir.join("README.md").is_file() || task_dir.join("verify.toml").is_file() {
        return Ok(task_dir);
    }
    Ok(std::env::current_dir()?)
}

/// `--progress` > `PROGRESS_FILE` > `/progress/progress.md`. An empty string disables the check.
pub fn resolve_progress(explicit: Option<String>) -> String {
    let env = std::env::var("PROGRESS_FILE").ok();
    resolve_progress_explicit_or_env(explicit, env.as_deref())
}

/// Pure core of `resolve_progress` (testable without touching process state).
fn resolve_progress_explicit_or_env(explicit: Option<String>, env: Option<&str>) -> String {
    if let Some(progress) = explicit {
        return progress;
    }
    if let Some(env_progress) = env {
        return env_progress.to_string();
    }
    "/progress/progress.md".to_string()
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
        // pure path: the env read is factored out so no test touches process state
        assert_eq!(
            resolve_progress_explicit_or_env(None, Some("/run/progress.md")),
            "/run/progress.md"
        );
        assert_eq!(
            resolve_progress_explicit_or_env(None, None),
            "/progress/progress.md"
        );
        assert_eq!(
            resolve_progress_explicit_or_env(Some(String::new()), None),
            ""
        );
    }

    #[test]
    fn redact_gate_output_never_leaks() {
        crate::redact::register("super-secret-token-value");
        let text = "CHECK focused.1 FAIL rc=1 super-secret-token-value\n";
        assert!(!crate::redact::scrub(text).contains("super-secret-token-value"));
    }
}
