//! Secret resolution for `env_secret` references.
//!
//! Two schemes are accepted, and both end the same way: the resolved value is registered with the
//! redactor before it is returned, and is never logged, echoed or written anywhere except the 0600
//! env-file consumed by `docker run --env-file`.
//!
//! - `op://…` — 1Password. The reference travels on argv (it is not a secret) and the value is
//!   captured through a pipe.
//! - `file://NAME` — a local 0600 file lying directly inside `$HOME/.config/taskfmt/`. No
//!   subprocess, so it cannot stall an unattended chain behind a locked desktop app.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;

use crate::redact;

/// Read one secret reference. The value is registered with the redactor before being returned.
///
/// `file://NAME` names a local credential file; every other reference is handed to `op read`.
pub fn read(reference: &str) -> anyhow::Result<String> {
    match reference.strip_prefix("file://") {
        Some(name) => read_local(name),
        None => read_op(reference),
    }
}

/// Resolve a whole `env_secret` map, registering every value.
pub fn resolve_all(
    references: &std::collections::BTreeMap<String, String>,
) -> anyhow::Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for (key, reference) in references {
        let value = read(reference).with_context(|| format!("resolving env_secret {key}"))?;
        out.push((key.clone(), value));
    }
    Ok(out)
}

/// `op://` resolution: shell out to the 1Password CLI.
fn read_op(reference: &str) -> anyhow::Result<String> {
    let mut cmd = Command::new("op");
    cmd.args(["read", "--no-newline", reference]);
    let output = cmd
        .output()
        .with_context(|| format!("cannot run `op read` for {}", label(reference)))?;
    if !output.status.success() {
        // stderr is deliberately not printed: `op` diagnostics can echo resolved material.
        anyhow::bail!(
            "`op read` failed for {} (rc={}). Unlock the 1Password desktop app (or run `eval $(op signin)`) and retry.",
            label(reference),
            output.status.code().unwrap_or(-1)
        );
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        anyhow::bail!("`op read` returned an empty value for {}", label(reference));
    }
    redact::register(&value);
    Ok(value)
}

/// The one directory a `file://` reference may name, and nothing else.
fn local_base() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("cannot resolve a file:// reference: HOME is unset")?;
    // `$HOME/.config` is canonicalized exactly ONCE. That buys tolerance for a symlinked HOME (or a
    // symlinked `.config`) without weakening the containment test below, because the final path
    // component is appended afterwards and stays literal.
    let cfg = std::fs::canonicalize(home.join(".config")).with_context(|| {
        format!(
            "cannot resolve a file:// reference: {} does not resolve",
            home.join(".config").display()
        )
    })?;

    // ---------------------------------------------------------------------------------------
    // `base` IS DELIBERATELY NOT CANONICALIZED. It must never become so.
    //
    // The containment test in `read_local_in` is
    //     canonicalize(base.join(NAME)).parent() == base
    // and it is sound only because the left-hand side resolves symlinks while the right-hand side
    // does not. Canonicalizing `base` would resolve a symlinked `taskfmt` directory as well; both
    // sides would then agree on the symlink's TARGET, and the check would ADMIT it.
    //
    // That is the exact escape this design exists to close. `~/.config/taskfmt` pointed at a
    // directory inside the `task-format` or `task-format-selfhost` worktree puts the credential
    // inside a repository that is pushed to GitHub, where disclosure is permanent and is not undone
    // by a later commit. Refusing a symlink AT `taskfmt` is the whole point of leaving this join
    // literal, and it costs nothing that the single canonicalize above has not already paid for.
    //
    // Consequence for the tests, and it is a trap rather than a detail: the legitimate-ADMIT
    // control must canonicalize its `tempfile::tempdir()` path BEFORE passing it in as `base`,
    // never the other way round. `tempdir()` hands back `/var/folders/…`, which canonicalizes to
    // `/private/var/folders/…`; a control that passes the raw path fails, and the obvious "fix" —
    // canonicalizing `base` here — is precisely the repair that destroys the design.
    // `a_base_that_is_itself_a_symlink_is_refused` is the detector for that repair.
    // ---------------------------------------------------------------------------------------
    let base = cfg.join("taskfmt");
    Ok(base)
}

/// Resolve `file://NAME` against the real `$HOME/.config/taskfmt`.
fn read_local(name: &str) -> anyhow::Result<String> {
    let base = local_base()?;
    read_local_in(name, &base)
}

/// The `file://` predicate, factored out of [`read_local`] so tests can drive it against a
/// `tempfile::tempdir()` instead of the operator's real home directory.
///
/// `base` is taken as given and is **not** canonicalized here — see [`local_base`] for why, and for
/// the obligation that puts on every caller and every test.
fn read_local_in(name: &str, base: &Path) -> anyhow::Result<String> {
    if name.is_empty() {
        anyhow::bail!("refusing an empty file:// reference: expected file://NAME");
    }
    if name.contains('/') {
        anyhow::bail!(
            "refusing file://{name}: the reference must be a bare file name, with no `/`"
        );
    }
    if name == ".." || name == "." {
        anyhow::bail!("refusing file://{name}: `.` and `..` are not credential file names");
    }

    let path = std::fs::canonicalize(base.join(name)).with_context(|| {
        format!(
            "refusing file://{name}: no such credential file in {}",
            base.display()
        )
    })?;

    if path.parent() != Some(base) {
        anyhow::bail!(
            "refusing file://{name}: it resolves to {}, which is not directly inside {}. \
             A credential must be a real file in that directory and must never be a symlink \
             pointing out of it.",
            path.display(),
            base.display()
        );
    }

    let meta = std::fs::metadata(&path)
        .with_context(|| format!("refusing file://{name}: cannot stat the credential file"))?;
    if !meta.is_file() {
        anyhow::bail!("refusing file://{name}: not a regular file");
    }
    check_mode_0600(&meta, name)?;

    let value = std::fs::read_to_string(&path)
        .with_context(|| format!("refusing file://{name}: the credential file is not valid UTF-8"))?
        .trim()
        .to_string();
    if value.is_empty() {
        anyhow::bail!("refusing file://{name}: the credential file is empty");
    }
    redact::register(&value);
    Ok(value)
}

/// A credential file must be exactly 0600. Anything wider is refused rather than warned about.
#[cfg(unix)]
fn check_mode_0600(meta: &std::fs::Metadata, name: &str) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = meta.permissions().mode() & 0o777;
    if mode != 0o600 {
        anyhow::bail!(
            "refusing file://{name}: mode is {mode:04o}, must be exactly 0600 (`chmod 600` it)"
        );
    }
    Ok(())
}

/// Fail closed where the mode cannot be read at all.
#[cfg(not(unix))]
fn check_mode_0600(_meta: &std::fs::Metadata, name: &str) -> anyhow::Result<()> {
    anyhow::bail!("refusing file://{name}: a 0600 mode cannot be verified on this platform")
}

/// A safe label for diagnostics: the vault/item path, never the value.
fn label(reference: &str) -> String {
    reference.split('/').take(5).collect::<Vec<_>>().join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Canonicalize the tempdir BEFORE it is used as `base`. Mandatory, and never the other way
    /// round: `tempdir()` returns `/var/folders/…` on macOS, which canonicalizes to
    /// `/private/var/folders/…`. See the comment block in `local_base`.
    fn canonical_base(dir: &tempfile::TempDir) -> PathBuf {
        std::fs::canonicalize(dir.path()).unwrap()
    }

    fn write_mode(path: &Path, body: &str, mode: u32) {
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    /// Plant a well-formed credential and assert it admits. Every refusal test calls this first, so
    /// no refusal can pass merely because the fixture was broken.
    fn admit_control(base: &Path, value: &str) {
        write_mode(&base.join("control.token"), &format!("{value}\n"), 0o600);
        assert_eq!(
            read_local_in("control.token", base).unwrap(),
            value,
            "the ADMIT control must admit, or the refusal beside it proves nothing"
        );
    }

    #[test]
    fn label_never_contains_the_value_field() {
        // The last segment of an op:// reference can be a field name, not a secret; everything
        // after the item is dropped so a secret-shaped reference never lands in a log.
        assert_eq!(
            label("op://vault/item/section/field"),
            "op://vault/item/section"
        );
        assert_eq!(label("op://vault/item/field"), "op://vault/item/field");
    }

    #[test]
    fn a_0600_file_directly_in_base_is_admitted_and_registered() {
        let dir = tempfile::tempdir().unwrap();
        let base = canonical_base(&dir);
        // unique per test: the redactor registry is global and shared across parallel tests, so a
        // length or membership assertion on a shared value would race (see redact.rs:139-141).
        let value = "tok-admit-c41f9b2e-directly-in-base";
        write_mode(
            &base.join("zai-flash.token"),
            &format!("  {value}\n  "),
            0o600,
        );

        let got = read_local_in("zai-flash.token", &base).unwrap();
        assert_eq!(got, value, "the value must be trimmed");
        assert!(
            redact::registered().contains(&value.to_string()),
            "an admitted credential MUST be registered with the redactor"
        );
    }

    #[test]
    fn a_name_containing_a_slash_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let base = canonical_base(&dir);
        admit_control(&base, "tok-control-slash-7a2d");

        for name in ["sub/zai.token", "../zai.token", "/etc/passwd"] {
            let err = read_local_in(name, &base).unwrap_err().to_string();
            assert!(err.contains("bare file name"), "{name}: {err}");
        }
    }

    #[test]
    fn a_dot_dot_name_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let base = canonical_base(&dir);
        admit_control(&base, "tok-control-dotdot-9e13");

        let err = read_local_in("..", &base).unwrap_err().to_string();
        assert!(err.contains("not credential file names"), "{err}");
    }

    #[test]
    fn a_dot_name_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let base = canonical_base(&dir);
        admit_control(&base, "tok-control-dot-5b8c");

        let err = read_local_in(".", &base).unwrap_err().to_string();
        assert!(err.contains("not credential file names"), "{err}");
    }

    #[test]
    fn an_empty_name_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let base = canonical_base(&dir);
        admit_control(&base, "tok-control-empty-name-3f60");

        let err = read_local_in("", &base).unwrap_err().to_string();
        assert!(err.contains("empty file:// reference"), "{err}");
    }

    #[test]
    fn a_directory_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let base = canonical_base(&dir);
        admit_control(&base, "tok-control-directory-2c47");

        std::fs::create_dir(base.join("zai-flash.token")).unwrap();
        let err = read_local_in("zai-flash.token", &base)
            .unwrap_err()
            .to_string();
        assert!(err.contains("not a regular file"), "{err}");
    }

    #[test]
    fn a_missing_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let base = canonical_base(&dir);
        admit_control(&base, "tok-control-missing-8d05");

        let err = read_local_in("absent.token", &base)
            .unwrap_err()
            .to_string();
        assert!(err.contains("no such credential file"), "{err}");
    }

    #[test]
    fn an_empty_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let base = canonical_base(&dir);
        admit_control(&base, "tok-control-empty-file-6a91");

        write_mode(&base.join("zai-flash.token"), "   \n\t\n", 0o600);
        let err = read_local_in("zai-flash.token", &base)
            .unwrap_err()
            .to_string();
        assert!(err.contains("is empty"), "{err}");
    }

    #[test]
    fn mode_0644_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let base = canonical_base(&dir);
        admit_control(&base, "tok-control-mode-4e72");

        // the SAME bytes at 0600 admit and at 0644 refuse: the mode is doing the work, not the file
        let value = "tok-mode-probe-b3f8";
        let path = base.join("zai-flash.token");
        write_mode(&path, &format!("{value}\n"), 0o600);
        assert_eq!(read_local_in("zai-flash.token", &base).unwrap(), value);

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let err = read_local_in("zai-flash.token", &base)
            .unwrap_err()
            .to_string();
        assert!(err.contains("must be exactly 0600"), "{err}");
        assert!(err.contains("0644"), "{err}");
    }

    #[test]
    fn a_leaf_symlink_pointing_outside_base_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = canonical_base(&dir);
        let base = root.join("base");
        let outside = root.join("outside");
        std::fs::create_dir(&base).unwrap();
        std::fs::create_dir(&outside).unwrap();
        admit_control(&base, "tok-control-leaf-symlink-1d9a");

        // the target is a perfectly valid credential when reached through its own directory
        let value = "tok-leaf-symlink-target-f207";
        write_mode(
            &outside.join("zai-flash.token"),
            &format!("{value}\n"),
            0o600,
        );
        assert_eq!(read_local_in("zai-flash.token", &outside).unwrap(), value);

        // reached through a symlink in `base` it is refused, because it does not LIVE in `base`
        std::os::unix::fs::symlink(
            outside.join("zai-flash.token"),
            base.join("zai-flash.token"),
        )
        .unwrap();
        let err = read_local_in("zai-flash.token", &base)
            .unwrap_err()
            .to_string();
        assert!(err.contains("not directly inside"), "{err}");
    }

    #[test]
    fn a_base_that_is_itself_a_symlink_is_refused() {
        // THE DETECTOR for the one bad repair. If anybody ever "fixes" a failing ADMIT control by
        // canonicalizing `base` inside `read_local_in`, this test is what fails: a `taskfmt`
        // directory symlinked into a pushed worktree would otherwise be admitted, which is exactly
        // how a credential reaches GitHub. Do not relax it; canonicalize the tempdir instead.
        let dir = tempfile::tempdir().unwrap();
        let root = canonical_base(&dir);
        let real = root.join("elsewhere");
        std::fs::create_dir(&real).unwrap();

        let value = "tok-base-symlink-victim-0c5e";
        write_mode(&real.join("zai-flash.token"), &format!("{value}\n"), 0o600);

        // control: through its real directory the very same file admits
        assert_eq!(read_local_in("zai-flash.token", &real).unwrap(), value);

        // through a symlinked base directory it must refuse
        let link = root.join("taskfmt");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let err = read_local_in("zai-flash.token", &link)
            .unwrap_err()
            .to_string();
        assert!(err.contains("not directly inside"), "{err}");
    }
}
