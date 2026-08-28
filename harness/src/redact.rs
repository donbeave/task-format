//! Global secret redactor.
//!
//! Every resolved secret (`op read` result) is registered here. All output the CLI forwards or
//! prints, every artifact file it writes, and every JSON manifest it emits passes through
//! [`scrub`] first, so a resolved secret can never survive into a log, a manifest, or the terminal.

use std::io::Write;
use std::path::Path;
use std::sync::{OnceLock, RwLock};

const MASK: &str = "[REDACTED]";
/// Secrets shorter than this are not registered: replacing a 2-3 character string would mangle
/// unrelated output while protecting nothing.
const MIN_LEN: usize = 6;

static REGISTRY: OnceLock<RwLock<Vec<String>>> = OnceLock::new();

fn registry() -> &'static RwLock<Vec<String>> {
    REGISTRY.get_or_init(|| RwLock::new(Vec::new()))
}

/// Eagerly create the registry. Called first thing in `main`.
pub fn init() {
    let _ = registry();
}

/// Register a secret so it is masked in every subsequent output or artifact.
pub fn register<S: AsRef<str>>(secret: S) {
    let secret = secret.as_ref();
    if secret.len() < MIN_LEN || secret.contains(MASK) {
        return;
    }
    let mut guard = registry().write().unwrap_or_else(|e| e.into_inner());
    if !guard.iter().any(|known| known == secret) {
        guard.push(secret.to_string());
    }
}

/// Register every entry of a resolved secret map (e.g. a profile's `env_secret` values).
pub fn register_all<I, S>(secrets: I)
where
    I: IntoIterator<Item = (S, S)>,
    S: AsRef<str>,
{
    for (_key, value) in secrets {
        register(value);
    }
}

/// Currently registered secrets. Diagnostics only; never printed.
pub fn registered() -> Vec<String> {
    registry().read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Replace every registered secret in `input` with `[REDACTED]`.
pub fn scrub(input: &str) -> String {
    let secrets = registered();
    if secrets.is_empty() {
        return input.to_string();
    }
    let mut out = input.to_string();
    for secret in &secrets {
        if out.contains(secret.as_str()) {
            out = out.replace(secret.as_str(), MASK);
        }
    }
    out
}

/// Scrub a byte buffer (log tails, copied agent streams).
pub fn scrub_bytes(input: &[u8]) -> Vec<u8> {
    match std::str::from_utf8(input) {
        Ok(text) => scrub(text).into_bytes(),
        Err(_) => scrub(&String::from_utf8_lossy(input)).into_bytes(),
    }
}

/// Print one scrubbed line to stdout.
pub fn emit(msg: &str) {
    println!("{}", scrub(msg));
}

/// Print one scrubbed line to stderr.
pub fn eemit(msg: &str) {
    eprintln!("{}", scrub(msg));
}

/// Print several scrubbed lines to stdout.
pub fn emit_lines(lines: impl IntoIterator<Item = impl AsRef<str>>) {
    for line in lines {
        emit(line.as_ref());
    }
}

/// Write `bytes` to `path`, scrubbed. Used for every artifact the harness stores
/// (manifests, gate logs, screen snapshots, copied agent output).
pub fn write_scrubbed(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let clean = scrub_bytes(bytes);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(path)?;
    file.write_all(&clean)
}

/// Serialize a JSON value and write it scrubbed (one document per file).
pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    let mut body = serde_json::to_vec_pretty(value)?;
    body.push(b'\n');
    write_scrubbed(path, &body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_secret_is_masked_in_text() {
        let secret = format!("sk-super-secret-{}", uuid::Uuid::new_v4());
        register(&secret);
        let out = scrub(&format!("token={secret}; done"));
        assert_eq!(out, "token=[REDACTED]; done");
    }

    #[test]
    fn scrub_hits_written_files_and_json_strings() {
        let secret = format!("op-secret-value-{}", uuid::Uuid::new_v4());
        register(&secret);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manifest.json");
        let value = serde_json::json!({ "env": format!("VALUE={secret}") });
        write_json(&path, &value).unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(!written.contains(&secret), "secret leaked into artifact");
        assert!(written.contains(MASK));
    }

    #[test]
    fn short_values_are_not_registered() {
        // the registry is global and other tests add to it concurrently, so assert about this
        // value only — a length delta would race with them
        register("abc");
        assert!(!registered().contains(&"abc".to_string()));
    }
}
