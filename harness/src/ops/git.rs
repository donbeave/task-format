//! git plumbing the harness needs: clone/fetch of the experiment repo, the trusted base commit,
//! the scope diff, and signed commits.

use std::collections::HashMap;
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

pub(super) fn in_dir(dir: &Path, args: &[&str]) -> Command {
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

/// Stage the complete worktree for an immutable candidate tree. Unlike [`add_all`], this includes
/// ignored untracked files: a verifier must see precisely the tree a later promotion could push.
/// `--` makes the repository root an explicit pathspec rather than letting a path named like an
/// option alter Git's parsing.
pub fn add_all_including_ignored(dir: &Path) -> anyhow::Result<()> {
    output(&mut in_dir(dir, &["add", "--all", "--force", "--", "."]))?;
    Ok(())
}

/// Write the current index as a Git tree object and return its immutable object ID.
pub fn write_tree(dir: &Path) -> anyhow::Result<String> {
    output(&mut in_dir(dir, &["write-tree"])).map(|tree| tree.trim().to_string())
}

/// A detached checkout of a tree frozen by a caller.  It owns the temporary worktree and removes
/// it from Git's worktree registry on drop.
pub struct DetachedWorktree {
    repo: PathBuf,
    _dir: tempfile::TempDir,
    path: PathBuf,
}

impl DetachedWorktree {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DetachedWorktree {
    fn drop(&mut self) {
        // Best effort only: failure to remove a temporary checkout must never replace the gate
        // result. `TempDir` will still remove its parent when possible.
        let _ = output(&mut in_dir(
            &self.repo,
            &[
                "worktree",
                "remove",
                "--force",
                &self.path.to_string_lossy(),
            ],
        ));
    }
}

/// Materialize exactly `tree` with exactly `parent` in a detached temporary worktree.  The
/// synthetic commit is intentionally unreachable: it exists only to give `git worktree add` a
/// stable commit object, while promotion later creates its own recorded commit from this tree.
pub fn detached_tree_worktree(
    repo: &Path,
    tree: &str,
    parent: &str,
) -> anyhow::Result<DetachedWorktree> {
    let candidate = commit_tree(
        repo,
        tree,
        parent,
        "taskfmt temporary gate candidate",
        false,
    )?;
    let dir = tempfile::Builder::new()
        .prefix("taskfmt-gate-")
        .tempdir()
        .context("creating temporary gate worktree directory")?;
    let path = dir.path().join("candidate");
    output(&mut in_dir(
        repo,
        &[
            "worktree",
            "add",
            "--detach",
            &path.to_string_lossy(),
            &candidate,
        ],
    ))
    .context("creating detached frozen candidate worktree")?;
    Ok(DetachedWorktree {
        repo: repo.to_path_buf(),
        _dir: dir,
        path,
    })
}

/// Create a commit directly from `tree` and its one recorded `parent`; this never reads HEAD or
/// the worktree. `sign` appends the same DCO trailer as `git commit -s` using the harness identity.
pub fn commit_tree(
    dir: &Path,
    tree: &str,
    parent: &str,
    message: &str,
    sign: bool,
) -> anyhow::Result<String> {
    let message = if sign {
        signed_message(message, "harness", "harness@localhost")
    } else {
        message.to_string()
    };
    let mut cmd = in_dir_as(
        dir,
        "harness",
        "harness@localhost",
        &["commit-tree", tree, "-p", parent, "-m"],
    );
    cmd.arg(message);
    output(&mut cmd).map(|commit| commit.trim().to_string())
}

/// Push this exact commit to `main`, refusing if the remote main is no longer `expected_parent`.
/// The source is an object ID rather than a branch name, so later movement of local `main` cannot
/// change the promoted commit.
pub fn push_main_with_lease(
    dir: &Path,
    remote: &str,
    commit: &str,
    expected_parent: &str,
) -> anyhow::Result<Captured> {
    let lease = format!("--force-with-lease=refs/heads/main:{expected_parent}");
    let refspec = format!("{commit}:refs/heads/main");
    check(
        &mut in_dir(dir, &["push", remote, &lease, &refspec]),
        "git push with main lease",
    )
}

/// The current object ID of `<remote>/main`, if that ref exists. Used only to recover a push that
/// succeeded before the local run record could be finalized.
pub fn remote_main(dir: &Path, remote: &str) -> anyhow::Result<Option<String>> {
    let out = output(&mut in_dir(dir, &["ls-remote", remote, "refs/heads/main"]))?;
    Ok(out
        .split_whitespace()
        .next()
        .filter(|oid| !oid.is_empty())
        .map(str::to_string))
}

fn signed_message(message: &str, name: &str, email: &str) -> String {
    let signoff = format!("Signed-off-by: {name} <{email}>");
    if message.lines().any(|line| line == signoff) {
        return message.to_string();
    }
    let mut signed = message.trim_end().to_string();
    if !signed.is_empty() {
        // A one-line message needs a body separator before its first trailer. A message that
        // already has a body/trailer block needs only one newline so provenance and DCO remain
        // one final trailer block.
        signed.push_str(if signed.contains("\n\n") {
            "\n"
        } else {
            "\n\n"
        });
    }
    signed.push_str(&signoff);
    signed.push('\n');
    signed
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

/// Fail-closed enumeration of every path the executor touched, vs `base`:
/// - `--no-renames`: a rename out->in must surface the deleted out-of-scope path, not just the
///   new one;
/// - untracked files honour only per-directory `.gitignore` (not `.git/info/exclude` or
///   `core.excludesFile`, both writable by the executor);
/// - an untracked `.gitignore` that ignores itself (`*`) is listed anyway, so a new ignore file
///   cannot hide its siblings.
/// - a candidate of that last enumeration is dropped only when its CONTAINING DIRECTORY was
///   already ignored by the ignore files as they exist in `base`. That is sound because if the
///   base ignored the directory, every untracked path under it was outside the scope check before
///   the run started, so no ignore file planted there can move a path from inside the check to
///   outside it. Testing the candidate itself would not be: a base rule naming ignore files could
///   then suppress one planted in a directory that is fully in scope. Any evaluator error keeps
///   every candidate.
///
/// Sorted and deduped.
pub fn changed_files(dir: &Path, base: &str) -> anyhow::Result<Vec<String>> {
    let mut files: Vec<String> = Vec::new();
    for (index, args) in [
        vec!["diff", "--no-renames", "--name-only", base, "--"],
        vec!["diff", "--no-renames", "--name-only", "--cached", "--"],
        vec!["ls-files", "--others", "--exclude-per-directory=.gitignore"],
        vec!["ls-files", "--others", "--", ":(top,glob)**/.gitignore"],
    ]
    .into_iter()
    .enumerate()
    {
        let out = output(&mut in_dir(dir, &args))?;
        let listed: Vec<String> = out
            .lines()
            .map(str::to_string)
            .filter(|line| !line.is_empty())
            .collect();
        if index == UNFILTERED_IGNORE_SCAN {
            files.extend(drop_dirs_ignored_at_base(dir, base, listed));
        } else {
            files.extend(listed);
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// Index, in `changed_files`' list, of the enumeration that deliberately applies no ignore
/// filtering of its own and therefore lists ignore files the base commit already ignored.
const UNFILTERED_IGNORE_SCAN: usize = 3;

/// Drop a candidate only when its containing directory was ignored under `base`'s own ignore
/// rules; keep everything else. Fail-closed at every step, so any error keeps every candidate and
/// the gate over-reports rather than hides.
fn drop_dirs_ignored_at_base(dir: &Path, base: &str, candidates: Vec<String>) -> Vec<String> {
    if candidates.is_empty() {
        return candidates;
    }
    let Ok(rules) = super::ignore::BaseIgnores::load(dir, base) else {
        return candidates;
    };
    let mut decided: HashMap<PathBuf, bool> = HashMap::new();
    candidates
        .into_iter()
        .filter(|path| {
            let parent = match Path::new(path).parent() {
                Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
                // A candidate at the repository root has no containing directory to judge.
                _ => return true,
            };
            !*decided
                .entry(parent.clone())
                .or_insert_with(|| rules.dir_ignored_at_base(&parent).unwrap_or(false))
        })
        .collect()
}

/// Index entries whose flags make `git diff` blind to worktree edits: `S` = skip-worktree,
/// lowercase tag = assume-unchanged (`git ls-files -v` lines, verbatim).
pub fn hidden_index_entries(dir: &Path) -> anyhow::Result<Vec<String>> {
    let out = output(&mut in_dir(dir, &["ls-files", "-v"]))?;
    Ok(out
        .lines()
        .filter(|line| {
            let mut chars = line.chars();
            matches!(chars.next(), Some(c) if c == 'S' || c.is_ascii_lowercase())
                && chars.next() == Some(' ')
        })
        .map(str::to_string)
        .collect())
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

    #[test]
    fn changed_files_drops_only_base_ignored_gitignore_candidates() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        init_repo(&repo);
        std::fs::write(repo.join(".gitignore"), "runs/\n").unwrap();
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "base"]);

        // One candidate under a directory the base already ignored, one under a directory it did
        // not. Both are untracked, and the fourth enumeration lists both.
        super::super::write_file(&repo.join("runs/x/.gitignore"), "").unwrap();
        super::super::write_file(&repo.join("src/evil/.gitignore"), "").unwrap();

        let changed = changed_files(&repo, "HEAD").unwrap();

        assert!(
            changed.contains(&"src/evil/.gitignore".to_string()),
            "a planted ignore file outside the base's ignored set must still be reported: {changed:?}"
        );
        assert!(
            !changed.contains(&"runs/x/.gitignore".to_string()),
            "an ignore file the base commit already ignored must not be reported: {changed:?}"
        );
    }

    #[test]
    fn changed_files_still_lists_a_self_ignoring_untracked_gitignore() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        init_repo(&repo);
        std::fs::write(repo.join(".gitignore"), "*.log\n").unwrap();
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "base"]);

        // An ignore file that ignores itself and its siblings, planted in a directory the base
        // never ignored: the property the fourth enumeration exists for.
        super::super::write_file(&repo.join("src/legacy/sub/.gitignore"), "*\n").unwrap();
        super::super::write_file(&repo.join("src/legacy/sub/hidden.txt"), "x\n").unwrap();

        let changed = changed_files(&repo, "HEAD").unwrap();

        assert!(
            changed.contains(&"src/legacy/sub/.gitignore".to_string()),
            "a self-ignoring untracked ignore file must still be reported: {changed:?}"
        );
    }

    #[test]
    fn complete_candidate_tree_includes_every_worktree_state_once() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        init_repo(&repo);
        std::fs::write(repo.join(".gitignore"), "ignored.txt\n").unwrap();
        std::fs::write(repo.join("tracked.txt"), "base\n").unwrap();
        std::fs::write(repo.join("deleted.txt"), "delete me\n").unwrap();
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "base"]);
        let parent = head(&repo).unwrap();

        std::fs::write(repo.join("tracked.txt"), "unstaged\n").unwrap();
        std::fs::write(repo.join("staged.txt"), "staged\n").unwrap();
        git(&repo, &["add", "staged.txt"]);
        std::fs::write(repo.join("untracked.txt"), "untracked\n").unwrap();
        std::fs::write(repo.join("ignored.txt"), "ignored\n").unwrap();
        std::fs::remove_file(repo.join("deleted.txt")).unwrap();

        add_all_including_ignored(&repo).unwrap();
        let tree = write_tree(&repo).unwrap();
        let paths = git(&repo, &["ls-tree", "-r", "--name-only", &tree]);
        assert_eq!(
            paths.lines().collect::<Vec<_>>(),
            vec![
                ".gitignore",
                "ignored.txt",
                "staged.txt",
                "tracked.txt",
                "untracked.txt"
            ]
        );

        let commit = commit_tree(&repo, &tree, &parent, "TASK-1: exact tree", true).unwrap();
        assert_eq!(
            git(&repo, &["rev-parse", &format!("{commit}^{{tree}}")]),
            tree
        );
        assert!(
            git(&repo, &["log", "-1", "--format=%B", &commit])
                .ends_with("TASK-1: exact tree\n\nSigned-off-by: harness <harness@localhost>")
        );
    }

    #[test]
    fn detached_tree_worktree_keeps_the_recorded_candidate_when_workspace_moves() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        init_repo(&repo);
        std::fs::write(repo.join("tracked.txt"), "base\n").unwrap();
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "base"]);
        let parent = head(&repo).unwrap();

        std::fs::write(repo.join("tracked.txt"), "candidate\n").unwrap();
        add_all_including_ignored(&repo).unwrap();
        let tree = write_tree(&repo).unwrap();
        let frozen = detached_tree_worktree(&repo, &tree, &parent).unwrap();

        std::fs::write(repo.join("tracked.txt"), "later workspace mutation\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(frozen.path().join("tracked.txt")).unwrap(),
            "candidate\n"
        );
        assert_eq!(git(frozen.path(), &["rev-parse", "HEAD^{tree}"]), tree);
        assert!(
            !capture(&mut in_dir(frozen.path(), &["symbolic-ref", "-q", "HEAD"]))
                .unwrap()
                .ok(),
            "candidate checkout must be detached"
        );
        let checkout = frozen.path().to_path_buf();
        drop(frozen);
        assert!(
            !checkout.exists(),
            "temporary candidate checkout must be removed"
        );
    }

    #[test]
    fn leased_explicit_commit_push_ignores_local_main_and_rejects_remote_race() {
        let tmp = tempfile::tempdir().unwrap();
        let bare = tmp.path().join("remote.git");
        std::fs::create_dir_all(&bare).unwrap();
        git(&bare, &["init", "-q", "--bare", "-b", "main"]);
        let repo = tmp.path().join("repo");
        init_repo(&repo);
        std::fs::write(repo.join("base.txt"), "base\n").unwrap();
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "base"]);
        let parent = head(&repo).unwrap();
        remote_add(&repo, "origin", &format!("file://{}", bare.display())).unwrap();
        push_upstream(&repo, "origin", "main").unwrap();

        std::fs::write(repo.join("candidate.txt"), "candidate\n").unwrap();
        add_all_including_ignored(&repo).unwrap();
        let tree = write_tree(&repo).unwrap();
        let candidate = commit_tree(&repo, &tree, &parent, "TASK-1: candidate", true).unwrap();

        // Move local main after gating. The explicit object-ID refspec must still push candidate.
        git(
            &repo,
            &["commit", "--allow-empty", "-q", "-m", "local distraction"],
        );
        push_main_with_lease(&repo, "origin", &candidate, &parent).unwrap();
        assert_eq!(head(&bare).unwrap(), candidate);
        assert_eq!(
            git(&bare, &["rev-parse", &format!("{candidate}^{{tree}}")]),
            tree
        );

        // Gate a fresh child of the recorded remote commit, then let another writer move main.
        // Its lease would succeed absent that race.
        git(&repo, &["checkout", "-q", &candidate]);
        std::fs::write(repo.join("second-candidate.txt"), "candidate two\n").unwrap();
        add_all_including_ignored(&repo).unwrap();
        let second_tree = write_tree(&repo).unwrap();
        let second_candidate = commit_tree(
            &repo,
            &second_tree,
            &candidate,
            "TASK-1: second candidate",
            true,
        )
        .unwrap();
        let racer = tmp.path().join("racer");
        clone_main(&format!("file://{}", bare.display()), &racer).unwrap();
        std::fs::write(racer.join("race.txt"), "race\n").unwrap();
        git(&racer, &["config", "user.name", "racer"]);
        git(&racer, &["config", "user.email", "racer@localhost"]);
        git(&racer, &["add", "-A"]);
        git(&racer, &["commit", "-q", "-m", "race"]);
        git(&racer, &["push", "-q", "origin", "main"]);
        let error =
            push_main_with_lease(&repo, "origin", &second_candidate, &candidate).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("git push with main lease failed")
        );
        assert_ne!(head(&bare).unwrap(), second_candidate);
    }
}
