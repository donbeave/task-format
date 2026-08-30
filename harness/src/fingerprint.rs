// Content fingerprint of the harness crate: a SHA-256 over the *hash input set* — `Cargo.toml`,
// `Cargo.lock`, `build.rs` and every file under `src/` — which is exactly what the
// `harness-taskfmt` build stage receives, so the host binary and the binary baked into an image
// can compute the same value from the same sources.
//
// This file is `include!`d by `build.rs` **and** compiled into the library (`pub mod fingerprint`),
// so the value baked in at compile time and the value `taskfmt fingerprint --path` computes are
// produced by the same code by construction. Two consequences, both load-bearing:
//
//   * it may use only `std` and `sha2`, the crate's one shared build dependency;
//   * its comments must be `//`, never `//!`: `include!` splices it into `build.rs` below the
//     crate root, where an inner doc comment is a hard error.
//
// What the digest identifies is the *content of the input set*, not the bytes of either compiled
// binary: two trees differing only in a doc comment compile to identical binaries and fingerprint
// differently. The over-approximation is in the safe direction — it can refuse a dispatch whose
// two binaries happen to be identical, and it can never pass a dispatch whose sources differ.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// The single-file members of the hash input set, relative to the crate root.
pub const HASH_INPUT_FILES: [&str; 3] = ["Cargo.toml", "Cargo.lock", "build.rs"];

/// The one directory member of the hash input set, hashed in full and recursively.
pub const HASH_INPUT_DIR: &str = "src";

/// Every file of the hash input set rooted at `crate_dir`, as `(relative path, absolute path)`
/// pairs sorted by relative path. Relative paths always use `/`, so the digest does not depend on
/// where the tree sits or on the host's separator.
pub fn hash_inputs(crate_dir: &Path) -> std::io::Result<Vec<(String, PathBuf)>> {
    let mut inputs: Vec<(String, PathBuf)> = Vec::new();
    for name in HASH_INPUT_FILES {
        let path = crate_dir.join(name);
        if !path.is_file() {
            return Err(missing(&path));
        }
        inputs.push((name.to_string(), path));
    }
    let dir = crate_dir.join(HASH_INPUT_DIR);
    if !dir.is_dir() {
        return Err(missing(&dir));
    }
    collect(&dir, HASH_INPUT_DIR, &mut inputs)?;
    inputs.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(inputs)
}

/// The digest of the hash input set rooted at `crate_dir`, as 64 lowercase hex digits.
///
/// Each input contributes its relative path, a NUL, its length and its bytes, so neither a rename
/// nor a shift of bytes across a file boundary can leave the digest unchanged.
pub fn fingerprint(crate_dir: &Path) -> std::io::Result<String> {
    let mut hasher = Sha256::new();
    for (rel, path) in hash_inputs(crate_dir)? {
        let bytes = std::fs::read(&path).map_err(|err| annotate(&path, err))?;
        hasher.update(rel.as_bytes());
        hasher.update([0u8]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(hex(hasher.finalize().as_slice()))
}

/// Depth-first walk of one input directory, appending `(relative path, absolute path)` pairs.
fn collect(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> std::io::Result<()> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|err| annotate(dir, err))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<PathBuf>>>()
        .map_err(|err| annotate(dir, err))?;
    entries.sort();
    for path in entries {
        let name = match path.file_name().and_then(|name| name.to_str()) {
            Some(name) => name.to_string(),
            None => return Err(missing(&path)),
        };
        let rel = format!("{prefix}/{name}");
        if path.is_dir() {
            collect(&path, &rel, out)?;
        } else {
            out.push((rel, path));
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[(byte >> 4) as usize]));
        out.push(char::from(DIGITS[(byte & 0x0f) as usize]));
    }
    out
}

fn missing(path: &Path) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("{} is not part of a harness crate tree", path.display()),
    )
}

fn annotate(path: &Path, err: std::io::Error) -> std::io::Error {
    std::io::Error::new(err.kind(), format!("{}: {err}", path.display()))
}
