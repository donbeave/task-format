// build.rs — bake the producing Git commit into the binary.

fn main() {
    let crate_dir = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo"),
    );

    println!("cargo:rerun-if-env-changed=TASKFMT_GIT_COMMIT_SHA");
    watch_git(&crate_dir);
    let git_sha = std::env::var("TASKFMT_GIT_COMMIT_SHA")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| git_commit_sha(&crate_dir))
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=TASKFMT_GIT_COMMIT_SHA={git_sha}");
}

fn git_commit_sha(crate_dir: &std::path::Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .current_dir(crate_dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let sha = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(sha)
}

/// Re-run when HEAD or the ref it points to moves, so a rebuild after a commit cannot retain the
/// previous commit's version string from Cargo's build-script cache.
fn watch_git(crate_dir: &std::path::Path) {
    let Ok(output) = std::process::Command::new("git")
        .current_dir(crate_dir)
        .args(["rev-parse", "--git-dir"])
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return;
    }
    let git_dir = std::path::PathBuf::from(&raw);
    let git_dir = if git_dir.is_absolute() {
        git_dir
    } else {
        crate_dir.join(git_dir)
    };
    println!("cargo:rerun-if-changed={}", git_dir.join("HEAD").display());
    println!(
        "cargo:rerun-if-changed={}",
        git_dir.join("packed-refs").display()
    );
    let Ok(head) = std::fs::read_to_string(git_dir.join("HEAD")) else {
        return;
    };
    let Some(reference) = head.trim().strip_prefix("ref: ") else {
        return;
    };
    println!(
        "cargo:rerun-if-changed={}",
        git_dir.join(reference).display()
    );
}
