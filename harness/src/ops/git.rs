//! git plumbing the harness needs: clone/fetch of the experiment repo, the trusted base commit,
//! the scope diff, and signed commits.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;

use super::{Captured, capture, check};

/// Run git in `dir`, capture output, error on non-zero.
pub fn output(cmd: &mut Command) -> anyhow::Result<String> {
    let captured = capture(cmd)?;
    if !captured.ok() {
        anyhow::bail!(
            "git failed (rc={}): {}",
            captured.status,
            crate::redact::scrub(captured.stderr.trim_end())
        );
    }
    Ok(captured.stdout)
}

fn in_dir(dir: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir).args(args);
    cmd
}

/// `git -c user.name=… -c user.email=… <args>` so commits work on hosts and in tests without
/// global config. The config keys must precede the git subcommand.
fn in_dir_as(dir: &Path, name: &str, email: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir);
    cmd.arg("-c").arg(format!("user.name={name}"));
    cmd.arg("-c").arg(format!("user.email={email}"));
    cmd.args(args);
    cmd
}

pub fn is_repo(dir: &Path) -> bool {
    capture(&mut in_dir(dir, &["rev-parse", "--is-inside-work-tree"]))
        .map(|out| out.ok() && out.stdout.trim() == "true")
        .unwrap_or(false)
}

pub fn toplevel(dir: &Path) -> Option<PathBuf> {
    capture(&mut in_dir(dir, &["rev-parse", "--show-toplevel"]))
        .ok()
        .filter(|out| out.ok())
        .map(|out| PathBuf::from(out.stdout.trim()))
}

pub fn head(dir: &Path) -> anyhow::Result<String> {
    output(&mut in_dir(dir, &["rev-parse", "HEAD"])).map(|s| s.trim().to_string())
}

pub fn rev_parse(dir: &Path, rev: &str) -> anyhow::Result<String> {
    output(&mut in_dir(dir, &["rev-parse", rev])).map(|s| s.trim().to_string())
}

pub fn has_ref(dir: &Path, rev: &str) -> bool {
    capture(&mut in_dir(
        dir,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ],
    ))
    .map(|out| out.ok())
    .unwrap_or(false)
}

/// `git init -b main`: a fresh repo whose initial branch is already `main`, so the bootstrap
/// commit lands where the experiment expects it.
pub fn init(dir: &Path) -> anyhow::Result<()> {
    output(&mut in_dir(dir, &["init", "-q", "-b", "main"]))?;
    Ok(())
}

pub fn remote_add(dir: &Path, name: &str, url: &str) -> anyhow::Result<()> {
    output(&mut in_dir(dir, &["remote", "add", name, url]))?;
    Ok(())
}

/// `git push -u <remote> <branch>`: the first push, which also sets the upstream.
pub fn push_upstream(dir: &Path, remote: &str, branch: &str) -> anyhow::Result<Captured> {
    check(
        &mut in_dir(dir, &["push", "-u", remote, branch]),
        "git push",
    )
}

/// Clone `url` into `dst` (fresh, shallow off — the gate needs the base commit and the diff).
pub fn clone(url: &str, dst: &Path) -> anyhow::Result<Captured> {
    check(
        Command::new("git").args(["clone", "--no-checkout", url, &dst.to_string_lossy()]),
        "git clone",
    )
}

/// `git clone --branch main --single-branch` for the workspace: keeps history small but complete.
pub fn clone_main(url: &str, dst: &Path) -> anyhow::Result<Captured> {
    check(
        Command::new("git").args([
            "clone",
            "--branch",
            "main",
            "--single-branch",
            url,
            &dst.to_string_lossy(),
        ]),
        "git clone",
    )
}

/// Update an existing clone to `origin/main` (fetch + hard reset) without recreating it.
pub fn fetch_reset(dir: &Path) -> anyhow::Result<()> {
    output(&mut in_dir(dir, &["fetch", "origin", "--prune"]))?;
    output(&mut in_dir(dir, &["reset", "--hard", "origin/main"]))?;
    output(&mut in_dir(dir, &["clean", "-fdx"]))?;
    Ok(())
}

pub fn checkout(dir: &Path, rev: &str) -> anyhow::Result<()> {
    output(&mut in_dir(dir, &["checkout", rev]))?;
    Ok(())
}

pub fn add_all(dir: &Path) -> anyhow::Result<()> {
    output(&mut in_dir(dir, &["add", "-A"]))?;
    Ok(())
}

pub fn status_porcelain(dir: &Path) -> anyhow::Result<Vec<String>> {
    let out = output(&mut in_dir(dir, &["status", "--porcelain"]))?;
    Ok(out.lines().map(str::to_string).collect())
}

/// Commit everything staged. `sign` adds `-s` (DCO), matching the repo rule.
pub fn commit(dir: &Path, message: &str, sign: bool, allow_empty: bool) -> anyhow::Result<String> {
    let mut args: Vec<&str> = vec!["commit"];
    if sign {
        args.push("-s");
    }
    if allow_empty {
        args.push("--allow-empty");
    }
    args.push("-m");
    let mut cmd = in_dir_as(dir, "harness", "harness@localhost", &args);
    cmd.arg(message);
    // the harness identity, used only for the trusted base commit and bootstrap commits
    let _ = output(&mut cmd)?;
    head(dir)
}

pub fn tag(dir: &Path, name: &str) -> anyhow::Result<()> {
    output(&mut in_dir(dir, &["tag", name]))?;
    Ok(())
}

pub fn push(dir: &Path, remote: &str, refspec: &str) -> anyhow::Result<Captured> {
    check(&mut in_dir(dir, &["push", remote, refspec]), "git push")
}

/// Changed files vs `base`: diff + staged + untracked, sorted and deduped.
pub fn changed_files(dir: &Path, base: &str) -> anyhow::Result<Vec<String>> {
    let mut files: Vec<String> = Vec::new();
    for args in [
        vec!["diff", "--name-only", base, "--"],
        vec!["diff", "--name-only", "--cached", "--"],
        vec!["ls-files", "--others", "--exclude-standard"],
    ] {
        let out = output(&mut in_dir(dir, &args))?;
        files.extend(
            out.lines()
                .map(str::to_string)
                .filter(|line| !line.is_empty()),
        );
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// Signed-off-by line of `HEAD`, when present.
pub fn head_signoff(dir: &Path) -> anyhow::Result<Option<String>> {
    let body = output(&mut in_dir(dir, &["log", "-1", "--format=%B"]))?;
    Ok(body
        .lines()
        .find(|line| line.starts_with("Signed-off-by:"))
        .map(str::to_string))
}

/// Config the agent workspace needs so git never refuses to operate on a mounted tree.
pub fn safe_directory_config() -> &'static str {
    "[safe]\n\tdirectory = *\n"
}

/// Set `safe.directory = *` inside a `$HOME/.gitconfig` the container mounts at `/agent-home`.
pub fn write_safe_directory(home: &Path) -> anyhow::Result<()> {
    let path = home.join(".gitconfig");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.contains("[safe]") {
        return Ok(());
    }
    let mut body = existing;
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    body.push_str(safe_directory_config());
    super::write_file(&path, &body).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) -> String {
        output(&mut in_dir(dir, args)).unwrap().trim().to_string()
    }

    fn init_repo(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["config", "user.name", "t"]);
        git(dir, &["config", "user.email", "t@t"]);
    }

    #[test]
    fn clone_commit_and_changed_files_on_a_local_bare_remote() {
        let tmp = tempfile::tempdir().unwrap();
        let seed = tmp.path().join("seed");
        init_repo(&seed);
        std::fs::write(seed.join("a.txt"), "one\n").unwrap();
        git(&seed, &["add", "-A"]);
        git(&seed, &["commit", "-q", "-m", "one"]);

        let bare = tmp.path().join("remote.git");
        std::fs::create_dir_all(&bare).unwrap();
        git(&bare, &["init", "-q", "--bare", "-b", "main"]);
        git(
            &seed,
            &[
                "remote",
                "add",
                "origin",
                &format!("file://{}", bare.display()),
            ],
        );
        git(&seed, &["push", "-q", "origin", "main"]);

        let work = tmp.path().join("work");
        clone_main(&format!("file://{}", bare.display()), &work).unwrap();
        assert!(is_repo(&work));
        assert_eq!(head(&work).unwrap(), head(&seed).unwrap());

        std::fs::write(work.join("b.txt"), "two\n").unwrap();
        let changed = changed_files(&work, "HEAD").unwrap();
        assert_eq!(changed, vec!["b.txt".to_string()]);
        assert!(!head_signoff(&work).unwrap().is_some());

        add_all(&work).unwrap();
        let sha = commit(&work, "test: signed", true, false).unwrap();
        assert_eq!(sha, head(&work).unwrap());
        assert!(
            head_signoff(&work)
                .unwrap()
                .unwrap()
                .starts_with("Signed-off-by:")
        );
        // origin already points at the bare repo: clone_main set it
        push(&work, "origin", "main").unwrap();
        assert_eq!(head(&bare).unwrap(), sha);
    }

    #[test]
    fn init_remote_and_upstream_push_bootstrap_a_branchless_remote() {
        let tmp = tempfile::tempdir().unwrap();
        let bare = tmp.path().join("remote.git");
        std::fs::create_dir_all(&bare).unwrap();
        git(&bare, &["init", "-q", "--bare"]);

        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        init(&repo).unwrap();
        assert_eq!(git(&repo, &["symbolic-ref", "--short", "HEAD"]), "main");
        remote_add(&repo, "origin", &format!("file://{}", bare.display())).unwrap();
        let sha = commit(&repo, "bootstrap", true, true).unwrap();
        push_upstream(&repo, "origin", "main").unwrap();
        assert_eq!(head(&bare).unwrap(), sha);
        assert_eq!(
            git(
                &repo,
                &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]
            ),
            "origin/main"
        );
    }

    #[test]
    fn safe_directory_is_written_once() {
        let tmp = tempfile::tempdir().unwrap();
        write_safe_directory(tmp.path()).unwrap();
        write_safe_directory(tmp.path()).unwrap();
        let text = std::fs::read_to_string(tmp.path().join(".gitconfig")).unwrap();
        assert_eq!(text.matches("[safe]").count(), 1);
    }
}
