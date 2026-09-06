// build.rs — bake the crate's content fingerprint and producing Git commit into the binary.
//
// The algorithm is `src/fingerprint.rs`, included verbatim and compiled into the library as well
// (R-002): one function, so the script and the binary cannot drift. That file may use only `std`
// and `sha2`, which is why `sha2` is in `[build-dependencies]` as well as `[dependencies]`.
//
// The `cargo:rerun-if-changed` lines below come in two forms and both are required. Registering
// each input individually is necessary but not sufficient: a file ADDED or DELETED under `src/`
// belongs to no previously registered set, so the script would not re-run and the compiled
// constant would go stale against the live input set. Registering the directory `src` as well
// fires on both addition and deletion.

include!("src/fingerprint.rs");

fn main() {
    let crate_dir = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo"),
    );
    let inputs = hash_inputs(&crate_dir)
        .unwrap_or_else(|err| panic!("cannot enumerate the harness hash input set: {err}"));
    for (_, path) in &inputs {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!(
        "cargo:rerun-if-changed={}",
        crate_dir.join(HASH_INPUT_DIR).display()
    );
    let digest = fingerprint(&crate_dir)
        .unwrap_or_else(|err| panic!("cannot fingerprint the harness hash input set: {err}"));
    println!("cargo:rustc-env=TASKFMT_HARNESS_FINGERPRINT={digest}");

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
