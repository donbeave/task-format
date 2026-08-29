//! Thin wrappers over the external binaries the harness drives (git, docker, gh, op, herdr, bash).
//! Every captured stream is available raw for the caller and scrubbed the moment it is printed or
//! written to an artifact.

pub mod container;
pub mod docker;
pub mod gh;
pub mod git;
pub mod herdr;
pub mod ignore;
pub mod images;
pub mod op;
pub mod signals;
pub mod transcript;

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Captured result of one external command.
#[derive(Debug, Clone, Default)]
pub struct Captured {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Captured {
    pub fn ok(&self) -> bool {
        self.status == 0
    }

    /// The command line, for diagnostics. Never contains a secret: secrets travel by env-file.
    pub fn display_cmd(program: &str, args: &[String]) -> String {
        let mut out = String::from(program);
        for arg in args {
            out.push(' ');
            out.push_str(arg);
        }
        out
    }
}

/// Run a command, capturing stdout and stderr. Errors only when the binary cannot be spawned.
pub fn capture(cmd: &mut Command) -> std::io::Result<Captured> {
    let out = cmd.stderr(Stdio::piped()).stdout(Stdio::piped()).output()?;
    Ok(Captured {
        status: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    })
}

/// Run a command capturing output; error when it exits non-zero.
pub fn check(cmd: &mut Command, what: &str) -> anyhow::Result<Captured> {
    let shown = match cmd.get_args().last() {
        Some(last) => format!(
            "{} {} … {}",
            cmd.get_program().to_string_lossy(),
            args_joined(cmd),
            last.to_string_lossy()
        ),
        None => cmd.get_program().to_string_lossy().to_string(),
    };
    let captured =
        capture(cmd).map_err(|err| anyhow::anyhow!("{what}: cannot spawn {shown}: {err}"))?;
    if !captured.ok() {
        anyhow::bail!(
            "{what} failed (rc={}):\n{}",
            captured.status,
            crate::redact::scrub(if captured.stderr.trim().is_empty() {
                &captured.stdout
            } else {
                &captured.stderr
            })
            .trim_end()
        );
    }
    Ok(captured)
}

fn args_joined(cmd: &Command) -> String {
    cmd.get_args()
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Echo a command invocation when verbose mode is on (scrubbed).
pub fn trace(verbose: bool, cmd: &Command) {
    if verbose {
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        crate::redact::eemit(&format!(
            "+ {} {}",
            cmd.get_program().to_string_lossy(),
            args.join(" ")
        ));
    }
}

/// Write text to a file, creating parents.
pub fn write_file(path: &Path, contents: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(path)?;
    file.write_all(contents.as_bytes())?;
    Ok(())
}

/// Copy a directory tree recursively (like `cp -a` for regular files and dirs).
pub fn copy_tree(src: &Path, dst: &Path) -> anyhow::Result<()> {
    copy_tree_filtered(src, dst, &|_| true)
}

/// Copy a directory tree, skipping every entry whose relative path is rejected by `keep` —
/// rejecting a directory rejects its whole subtree.
pub fn copy_tree_filtered(
    src: &Path,
    dst: &Path,
    keep: &dyn Fn(&Path) -> bool,
) -> anyhow::Result<()> {
    let mut pruned: Vec<std::path::PathBuf> = Vec::new();
    for entry in walkdir::WalkDir::new(src).follow_links(false) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?.to_path_buf();
        if pruned.iter().any(|prefix| rel.starts_with(prefix)) {
            continue;
        }
        if !keep(&rel) {
            pruned.push(rel);
            continue;
        }
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
        } else if entry.file_type().is_symlink() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let target_link = std::fs::read_link(entry.path())?;
            let _ = std::fs::remove_file(&target);
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target_link, &target)?;
        }
    }
    Ok(())
}

/// Create a symlink (Unix only; the harness targets Linux containers and a macOS host).
#[cfg(unix)]
pub fn symlink(target: &Path, link: &Path) -> anyhow::Result<()> {
    if link.symlink_metadata().is_ok() {
        std::fs::remove_file(link)?;
    }
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

/// Files under `dir` (non-recursive) whose name matches `suffix`, byte-sorted (`LC_ALL=C sort`).
pub fn sorted_files_by_name(
    dir: &Path,
    predicate: &dyn Fn(&str) -> bool,
) -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .map(|name| predicate(&name.to_string_lossy()))
                    .unwrap_or(false)
        })
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_tree_keeps_the_symlink_shape() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), "hello").unwrap();
        std::fs::write(src.join("sub").join("b.txt"), "there").unwrap();
        symlink(Path::new("a.txt"), &src.join("CLAUDE.md")).unwrap();

        let dst = dir.path().join("dst");
        copy_tree(&src, &dst).unwrap();
        assert_eq!(std::fs::read_to_string(dst.join("a.txt")).unwrap(), "hello");
        assert_eq!(
            std::fs::read_to_string(dst.join("sub").join("b.txt")).unwrap(),
            "there"
        );
        let link = std::fs::read_link(dst.join("CLAUDE.md")).unwrap();
        assert_eq!(link, std::path::PathBuf::from("a.txt"));
    }

    #[test]
    fn copy_tree_filter_can_skip() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join(".git")).unwrap();
        std::fs::write(src.join(".git").join("HEAD"), "ref").unwrap();
        std::fs::write(src.join("keep.txt"), "x").unwrap();
        let dst = dir.path().join("dst");
        copy_tree_filtered(&src, &dst, &|rel| rel != Path::new(".git")).unwrap();
        assert!(dst.join("keep.txt").is_file());
        assert!(!dst.join(".git").exists());
    }

    #[test]
    fn sorted_files_filters_and_sorts() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.sql"), "").unwrap();
        std::fs::write(dir.path().join("a.sql"), "").unwrap();
        std::fs::write(dir.path().join("c.txt"), "").unwrap();
        let files = sorted_files_by_name(dir.path(), &|name| name.ends_with(".sql"));
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["a.sql", "b.sql"]);
    }
}
