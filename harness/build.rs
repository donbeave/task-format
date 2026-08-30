// build.rs — bake the crate's content fingerprint into the binary as `TASKFMT_HARNESS_FINGERPRINT`.
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
}
