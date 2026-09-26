//! "Were these ignore rules already in force at the trusted base commit?"
//!
//! git has no primitive for this. `git check-ignore` answers from the working tree — per-directory
//! `.gitignore` files, `.git/info/exclude` and `core.excludesFile` — and every one of those is
//! writable by the executor whose work the gate is judging, so none of them may inform the answer.
//!
//! The only reconstruction that does not reimplement git's pattern semantics is to read the base
//! commit's ignore blobs out of the object store, materialise them at their tree paths in a scratch
//! repository outside the judged worktree, and ask git there. Path queries add empty candidate files
//! and parent directories to preserve file-versus-directory semantics; candidate contents never
//! become rules. Thus the only ignore sources are the base commit's blobs.
//!
//! The scope caller treats every failure as "not ignored", which keeps the candidate in the
//! changed set. The gate then over-reports rather than hides, and a degraded evaluator reproduces
//! the visible symptom instead of opening a silent hole.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Component, Path};
use std::process::{Command, Stdio};

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
        let listing = super::git::output_bytes(&mut super::git::in_dir(
            root,
            &["ls-tree", "-r", "-z", base],
        ))?;
        let paths = listing
            .split(|byte| *byte == 0)
            .filter_map(ignore_blob_path)
            .map(|path| {
                std::str::from_utf8(path).map(str::to_string).map_err(|error| {
                    anyhow::anyhow!(
                        "base commit contains a `.gitignore` path that is not valid UTF-8 at byte {}; scope validation requires UTF-8 paths",
                        error.valid_up_to()
                    )
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
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
            let blob = super::git::output_bytes(&mut super::git::in_dir(
                root,
                &["show", &format!("{base}:{path}")],
            ))?;
            std::str::from_utf8(&blob).map_err(|error| {
                anyhow::anyhow!(
                    "base commit `.gitignore` {path:?} is not valid UTF-8 at byte {}; base ignore rules cannot be applied safely",
                    error.valid_up_to()
                )
            })?;
            // Placement reproduces per-directory relativity: a rule in `a/b/.gitignore` governs
            // `a/b` and below in the scratch exactly as it did in the base commit's tree.
            let file = here.join(path);
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(file, blob)?;
        }

        Ok(Self {
            scratch: Some(scratch),
        })
    }

    /// True when the root-relative directory `dir` is ignored under the base commit's rules.
    ///
    /// Unusable input — empty, absolute, or holding any component that is not a plain name — is
    /// answered `false` rather than guessed at.
    #[cfg(test)]
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

    /// For each root-relative untracked file, report whether the base commit's ignore rules
    /// ignore that file. Candidates are materialised as files before querying so a directory-only
    /// rule such as `build/` cannot hide an untracked file named `build`.
    ///
    /// The query is batched and atomic: an unusable path or failed git query returns an error, so
    /// callers can retain the complete candidate set rather than applying a partial answer.
    pub fn paths_ignored_at_base(&self, paths: &[String]) -> anyhow::Result<Vec<bool>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let Some(scratch) = self.scratch.as_ref() else {
            return Ok(vec![false; paths.len()]);
        };

        let mut valid = HashMap::<String, Vec<usize>>::new();
        for (index, path) in paths.iter().enumerate() {
            let Some(relative) = plain_relative_file(Path::new(path)) else {
                anyhow::bail!("unusable relative file path: {path:?}");
            };
            let file = scratch.path().join(&relative);
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            match OpenOptions::new().write(true).create_new(true).open(&file) {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if !file.is_file() {
                        anyhow::bail!("candidate is not a file in ignore scratch: {path:?}");
                    }
                }
                Err(error) => return Err(error.into()),
            }
            valid.entry(relative).or_default().push(index);
        }

        let mut input = tempfile::tempfile()?;
        for path in valid.keys() {
            input.write_all(path.as_bytes())?;
            input.write_all(b"\0")?;
        }
        input.seek(SeekFrom::Start(0))?;

        let captured = capture(&mut check_ignore_paths(scratch.path(), input))?;
        if captured.status != 0 && captured.status != 1 {
            anyhow::bail!(
                "git check-ignore failed (exit={}): {}",
                captured.status,
                captured.stderr.trim_end()
            );
        }

        let mut ignored = vec![false; paths.len()];
        if captured.status == 0 {
            if captured.stdout.is_empty() {
                anyhow::bail!("git check-ignore matched candidates without returning paths");
            }
            let fields = captured.stdout.split('\0').collect::<Vec<_>>();
            if fields.last() != Some(&"") {
                anyhow::bail!("git check-ignore returned malformed NUL-delimited output");
            }
            let (records, remainder) = fields[..fields.len() - 1].as_chunks::<4>();
            if !remainder.is_empty() {
                anyhow::bail!("git check-ignore returned malformed NUL-delimited output");
            }
            for record in records {
                let Some(indices) = valid.get(record[3]) else {
                    anyhow::bail!("git check-ignore returned an unknown candidate path");
                };
                for index in indices {
                    ignored[*index] = true;
                }
            }
        }
        Ok(ignored)
    }
}

/// The path of one `ls-tree -r -z` entry, when that entry is a regular-file ignore file.
///
/// Symlinks (`120000`) and gitlinks (`160000`) are skipped: their patterns never suppress anything,
/// which is the over-reporting direction.
fn ignore_blob_path(entry: &[u8]) -> Option<&[u8]> {
    let tab = entry.iter().position(|byte| *byte == b'\t')?;
    let meta = &entry[..tab];
    let path = &entry[tab + 1..];
    let mode = meta.split(|byte| byte.is_ascii_whitespace()).next()?;
    if mode != b"100644" && mode != b"100755" {
        return None;
    }
    if path == b".gitignore" || path.ends_with(b"/.gitignore") {
        Some(path)
    } else {
        None
    }
}

/// `dir` as a relative path of plain components, or `None` when it cannot be one.
#[cfg(test)]
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

/// `path` as a relative file made only of plain components.
fn plain_relative_file(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    if text.is_empty() || !path.components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(text.to_string())
}

/// Isolated directory query retained for the directory-rule unit cases.
///
/// `core.excludesFile=/dev/null` suppresses the per-user ignore file, because that file's default
/// location is a built-in fallback rather than a config value and so survives
/// `GIT_CONFIG_GLOBAL=/dev/null`; the emptied `info/exclude` in `load` suppresses the scratch's own
/// template rules.
#[cfg(test)]
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

/// One batch query for concrete file paths. `-z` preserves arbitrary path names, while the
/// explicit environment keeps ambient/global excludes out of the base-commit decision.
fn check_ignore_paths(scratch: &Path, input: std::fs::File) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(scratch)
        .args([
            "-c",
            "core.excludesFile=/dev/null",
            "check-ignore",
            "-v",
            "-z",
            "--no-index",
            "--stdin",
        ])
        .stdin(Stdio::from(input))
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

    fn paths_ignored(rules: &BaseIgnores, paths: &[&str]) -> Vec<bool> {
        rules
            .paths_ignored_at_base(
                &paths
                    .iter()
                    .map(|path| path.to_string())
                    .collect::<Vec<_>>(),
            )
            .unwrap()
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
    fn base_filename_pattern_ignores_untracked_file() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        base_repo(&repo, &[(".gitignore", "*.secret\n")]);
        let rules = BaseIgnores::load(&repo, "HEAD").unwrap();

        assert_eq!(
            paths_ignored(&rules, &["hidden.secret", "visible.txt"]),
            [true, false]
        );
    }

    #[test]
    fn malformed_base_ignore_blob_is_rejected_instead_of_lossily_decoded() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        base_repo(&repo, &[(".gitignore", "placeholder\n")]);
        std::fs::write(repo.join(".gitignore"), b"\xff\n").unwrap();
        git(&repo, &["add", "-f", ".gitignore"]);
        git(&repo, &["commit", "-q", "-m", "malformed ignore bytes"]);

        let error = match BaseIgnores::load(&repo, "HEAD") {
            Ok(_) => panic!("a malformed base ignore blob must not be decoded lossily"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("`.gitignore`"), "{error}");
        assert!(error.contains("not valid UTF-8"), "{error}");
    }

    #[test]
    fn a_directory_only_base_rule_does_not_ignore_a_same_named_file() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        base_repo(&repo, &[(".gitignore", "build/\n")]);
        let rules = BaseIgnores::load(&repo, "HEAD").unwrap();

        assert_eq!(paths_ignored(&rules, &["build"]), [false]);
    }

    #[test]
    fn a_directory_only_base_rule_ignores_a_descendant_file() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        base_repo(&repo, &[(".gitignore", "build/\n")]);
        let rules = BaseIgnores::load(&repo, "HEAD").unwrap();

        assert_eq!(paths_ignored(&rules, &["build/output.txt"]), [true]);
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
