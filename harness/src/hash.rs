//! SHA-256 digests for gate evidence and artifact identity.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::Context;
use sha2::{Digest, Sha256};

/// Lowercase hex SHA-256 of a byte slice.
pub fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Lowercase hex SHA-256 of a file's bytes exactly as written.
pub fn digest_file(path: &Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("cannot read {} to digest it", path.display()))?;
    Ok(digest_bytes(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_matches_published_vectors() {
        assert_eq!(
            digest_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            digest_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn file_and_bytes_agree() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("verdict.json");
        std::fs::write(&path, b"{\"overall\":\"PASS\"}").unwrap();
        assert_eq!(
            digest_file(&path).unwrap(),
            digest_bytes(b"{\"overall\":\"PASS\"}")
        );
    }
}
