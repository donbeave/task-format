//! "Were these ignore rules already in force at the trusted base commit?"
//!
//! git has no primitive for this. `git check-ignore` answers from the working tree — per-directory
//! `.gitignore` files, `.git/info/exclude` and `core.excludesFile` — and every one of those is
//! writable by the executor whose work the gate is judging, so none of them may inform the answer.
//!
//! The only reconstruction that does not reimplement git's pattern semantics is to read the base
//! commit's ignore blobs out of the object store, materialise them at their tree paths in a scratch
//! repository outside the judged worktree, and ask git there. The scratch contains nothing but those
//! blobs, so what it answers with is exactly the base commit's rules and nothing else.
//!
//! Every failure is answered "not ignored", which keeps the candidate in the changed set. The gate
//! then over-reports rather than hides, and a degraded evaluator reproduces the visible symptom
//! instead of opening a silent hole.

use std::path::{Component, Path};
use std::process::Command;

use super::{capture, write_file};

/// The base commit's ignore rules, materialised once and queried many times.
///
/// `load` pays one `ls-tree`, one `show` per ignore blob and one `init`; each query afterwards is a
/// single `check-ignore` in the scratch. Batching is what makes a failure atomic: `load` either
/// yields rules that answer for every candidate in a call, or fails and leaves every candidate
/// reported. A per-candidate form could fail part-way and produce an order-dependent partition of
/// one enumeration's output, which is exactly the inconsistency the scope check must never have.
pub struct BaseIgnores {
    /// `None` when the base commit carries no ignore file at all: nothing can be ignored, and no
    /// git process is ever spawned for a query.
    scratch: Option<tempfile::TempDir>,
}

impl BaseIgnores {
    /// Read every ignore file out of `base` and stage them for querying.
    ///
    /// The reads run against `root` with the ambient environment, exactly like the enumerations in
    /// `super::git::changed_files`, so a container relying on `safe.directory` in its git config is
    /// unaffected. Only the scratch queries are isolated.
    pub fn load(root: &Path, base: &str) -> anyhow::Result<Self> {
        let listing = super::git::output(&mut super::git::in_dir(
            root,
            &["ls-tree", "-r", "-z", base],
        ))?;
        let paths: Vec<&str> = listing.split('\0').filter_map(ignore_blob_path).collect();
        if paths.is_empty() {
            return Ok(Self { scratch: None });
        }

        let scratch = tempfile::tempdir()?;
        let here = std::fs::canonicalize(scratch.path())?;
        let judged = std::fs::canonicalize(root)?;
        if here.starts_with(&judged) {
            anyhow::bail!(
                "the scratch ignore repository would land inside the judged worktree ({})",
                judged.display()
            );
        }

        super::git::output(&mut super::git::in_dir(&here, &["init", "-q"]))?;
        // `git init` copies a template, and git reads the scratch's own `info/exclude`. A host
        // whose template carries rules would otherwise have them answer for the base commit.
        write_file(&here.join(".git").join("info").join("exclude"), "")?;
        for path in paths {
            let blob = super::git::output(&mut super::git::in_dir(
                root,
                &["show", &format!("{base}:{path}")],
            ))?;
            // Placement reproduces per-directory relativity: a rule in `a/b/.gitignore` governs
            // `a/b` and below in the scratch exactly as it did in the base commit's tree.
            write_file(&here.join(path), &blob)?;
        }

        Ok(Self {
            scratch: Some(scratch),
        })
    }

    /// True when the root-relative directory `dir` is ignored under the base commit's rules.
    ///
    /// Unusable input — empty, absolute, or holding any component that is not a plain name — is
    /// answered `false` rather than guessed at.
    pub fn dir_ignored_at_base(&self, dir: &Path) -> anyhow::Result<bool> {
        let Some(scratch) = self.scratch.as_ref() else {
            return Ok(false);
        };
        let Some(relative) = plain_relative_dir(dir) else {
            return Ok(false);
        };
        // Required, not incidental: a directory-only pattern (`x/`) matches the exact path `x` only
        // when git can see that `x` is a directory. Descendants match by ancestor propagation
        // without it, so omitting this would answer the exact-directory case wrongly.
        std::fs::create_dir_all(scratch.path().join(&relative))?;
        Ok(capture(&mut check_ignore(scratch.path(), &relative))?.status == 0)
    }
}

/// The path of one `ls-tree -r -z` entry, when that entry is a regular-file ignore file.
///
/// Symlinks (`120000`) and gitlinks (`160000`) are skipped: their patterns never suppress anything,
/// which is the over-reporting direction.
fn ignore_blob_path(entry: &str) -> Option<&str> {
    let (meta, path) = entry.split_once('\t')?;
    let mode = meta.split_whitespace().next()?;
    if mode != "100644" && mode != "100755" {
        return None;
    }
    if path == ".gitignore" || path.ends_with("/.gitignore") {
        Some(path)
    } else {
        None
    }
}

/// `dir` as a relative path of plain components, or `None` when it cannot be one.
fn plain_relative_dir(dir: &Path) -> Option<String> {
    let text = dir.to_str()?;
    if text.is_empty() {
        return None;
    }
    if !dir.components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(text.to_string())
}

/// The one query this module makes, with the isolation it needs.
///
/// Both levers are load-bearing and neither substitutes for the other: `core.excludesFile=/dev/null`
/// is what suppresses the per-user ignore file, because that file's default location is a built-in
/// fallback rather than a config value and so survives `GIT_CONFIG_GLOBAL=/dev/null`; the emptied
/// `info/exclude` in `load` is what suppresses the scratch's own template rules.
fn check_ignore(scratch: &Path, dir: &str) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(scratch)
        .args([
            "-c",
            "core.excludesFile=/dev/null",
            "check-ignore",
            "-q",
            "--no-index",
            "--",
            dir,
        ])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let captured = capture(&mut super::super::git::in_dir(dir, args)).unwrap();
        assert!(captured.ok(), "git {args:?} failed: {}", captured.stderr);
    }

    /// A repository holding one base commit built from `files`.
    fn base_repo(dir: &Path, files: &[(&str, &str)]) {
        std::fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["config", "user.name", "t"]);
        git(dir, &["config", "user.email", "t@t"]);
        for (path, contents) in files {
            write_file(&dir.join(path), contents).unwrap();
        }
        // `-f` because a base fixture may commit an ignore file that matches itself.
        git(dir, &["add", "-A", "-f"]);
        git(dir, &["commit", "-q", "-m", "base"]);
    }

    fn ignored(rules: &BaseIgnores, dir: &str) -> bool {
        rules.dir_ignored_at_base(Path::new(dir)).unwrap()
    }

    /// `git check-ignore` as the working tree answers it — the untrusted opinion the evaluator
    /// must not share.
    fn worktree_says_ignored(repo: &Path, dir: &str) -> bool {
        capture(&mut super::super::git::in_dir(
            repo,
            &["check-ignore", "-q", "--", dir],
        ))
        .unwrap()
        .status
            == 0
    }

    #[test]
    fn a_dir_ignored_by_a_base_root_rule_is_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        base_repo(&repo, &[(".gitignore", "runs/\n")]);
        let rules = BaseIgnores::load(&repo, "HEAD").unwrap();

        assert!(
            ignored(&rules, "runs/x"),
            "a descendant of a base-ignored directory"
        );
        assert!(
            ignored(&rules, "runs"),
            "the exact directory the base rule names; this is the case create_dir_all exists for"
        );
    }

    #[test]
    fn a_dir_no_base_rule_covers_is_not_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        base_repo(&repo, &[(".gitignore", "runs/\n")]);
        let rules = BaseIgnores::load(&repo, "HEAD").unwrap();

        assert!(!ignored(&rules, "src/evil"));
        assert!(!ignored(&rules, "src"));
    }

    #[test]
    fn a_worktree_only_rule_does_not_ignore() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        base_repo(&repo, &[(".gitignore", "runs/\n")]);
        // Written but never committed: the shape an executor can produce at will.
        write_file(&repo.join(".gitignore"), "runs/\nsrc/\n").unwrap();
        std::fs::create_dir_all(repo.join("src/evil")).unwrap();

        let rules = BaseIgnores::load(&repo, "HEAD").unwrap();

        assert!(
            worktree_says_ignored(&repo, "src/evil"),
            "control: the working tree does consider it ignored"
        );
        assert!(
            !ignored(&rules, "src/evil"),
            "the base commit is the only source that may decide"
        );
    }

    #[test]
    fn info_exclude_does_not_ignore() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        base_repo(&repo, &[(".gitignore", "runs/\n")]);
        write_file(&repo.join(".git").join("info").join("exclude"), "src/\n").unwrap();
        std::fs::create_dir_all(repo.join("src/evil")).unwrap();

        let rules = BaseIgnores::load(&repo, "HEAD").unwrap();

        assert!(
            worktree_says_ignored(&repo, "src/evil"),
            "control: info/exclude does reach the judged repository's own answer"
        );
        assert!(!ignored(&rules, "src/evil"));
    }

    #[test]
    fn a_scratch_template_exclude_does_not_ignore() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        base_repo(&repo, &[(".gitignore", "runs/\n")]);
        let rules = BaseIgnores::load(&repo, "HEAD").unwrap();
        let scratch = rules.scratch.as_ref().unwrap().path().to_path_buf();
        let exclude = scratch.join(".git").join("info").join("exclude");

        assert_eq!(
            std::fs::metadata(&exclude).unwrap().len(),
            0,
            "load must empty the scratch's own info/exclude"
        );

        // And that emptying is load-bearing, not decoration: git reads this file.
        assert!(!ignored(&rules, "src/evil"));
        write_file(&exclude, "src/\n").unwrap();
        assert!(
            ignored(&rules, "src/evil"),
            "control: a non-empty scratch info/exclude would decide, which is why load empties it"
        );
    }

    #[test]
    fn a_base_negation_reincludes() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        // `runs/*` rather than `runs/`: git cannot re-include anything under an excluded
        // directory, so the directory form would assert a semantics git does not have.
        base_repo(&repo, &[(".gitignore", "runs/*\n!runs/keep\n")]);
        let rules = BaseIgnores::load(&repo, "HEAD").unwrap();

        assert!(ignored(&rules, "runs/x"));
        assert!(
            !ignored(&rules, "runs/keep"),
            "the negation must be honoured"
        );
    }

    #[test]
    fn a_nested_base_ignore_file_applies_at_its_own_level() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        base_repo(&repo, &[("runs/.gitignore", "*\n")]);
        let rules = BaseIgnores::load(&repo, "HEAD").unwrap();

        assert!(ignored(&rules, "runs/x"), "governed by runs/.gitignore");
        assert!(
            !ignored(&rules, "runs"),
            "the rule does not govern its own directory"
        );
        assert!(
            !ignored(&rules, "src/evil"),
            "and does not escape its directory"
        );
    }

    #[test]
    fn the_self_ignoring_root_case_is_not_filtered() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        // The tamper-matrix shape: a base that ignores logs, and a run that plants an ignore file
        // containing `*` deep inside a directory the base never ignored.
        base_repo(&repo, &[(".gitignore", "*.log\n")]);
        write_file(&repo.join("src/legacy/sub/.gitignore"), "*\n").unwrap();
        let rules = BaseIgnores::load(&repo, "HEAD").unwrap();

        assert!(
            !ignored(&rules, "src/legacy/sub"),
            "a planted ignore file must never suppress the report of itself"
        );
    }

    #[test]
    fn a_base_with_no_ignore_file_ignores_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        base_repo(&repo, &[("a.txt", "one\n")]);
        let rules = BaseIgnores::load(&repo, "HEAD").unwrap();

        assert!(
            rules.scratch.is_none(),
            "no ignore blob means no scratch and no git"
        );
        assert!(!ignored(&rules, "runs"));
        assert!(!ignored(&rules, "anything/at/all"));
    }

    #[test]
    fn an_unusable_candidate_path_is_not_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        base_repo(&repo, &[(".gitignore", "runs/\n")]);
        let rules = BaseIgnores::load(&repo, "HEAD").unwrap();

        for candidate in ["", "/runs", "../runs", "runs/../runs", "./runs"] {
            assert!(
                !ignored(&rules, candidate),
                "{candidate:?} is not a usable relative directory and must be kept"
            );
        }
    }

    #[test]
    fn a_global_excludes_file_does_not_ignore() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        base_repo(&repo, &[(".gitignore", "runs/\n")]);
        let home = tmp.path().join("home");
        write_file(&home.join(".config").join("git").join("ignore"), "src/\n").unwrap();
        let xdg = home.join(".config");

        let rules = BaseIgnores::load(&repo, "HEAD").unwrap();
        let scratch = rules.scratch.as_ref().unwrap().path().to_path_buf();
        std::fs::create_dir_all(scratch.join("src")).unwrap();

        // Control: the same query without the lever lets the per-user file decide. The environment
        // is set on the child, so no other test's view of HOME is disturbed.
        let mut unguarded = Command::new("git");
        unguarded
            .current_dir(&scratch)
            .args(["check-ignore", "-q", "--no-index", "--", "src"])
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", &xdg);
        assert_eq!(
            capture(&mut unguarded).unwrap().status,
            0,
            "control: a per-user excludes file reaches an unguarded query"
        );

        let mut guarded = check_ignore(&scratch, "src");
        guarded.env("HOME", &home).env("XDG_CONFIG_HOME", &xdg);
        assert_ne!(
            capture(&mut guarded).unwrap().status,
            0,
            "core.excludesFile=/dev/null must keep the per-user file out of the decision"
        );
    }
}
