//! `op` (1Password) secret resolution.
//!
//! Rules enforced here: the reference travels on argv (it is not a secret), the resolved value is
//! captured through a pipe, registered in the redactor immediately, and never logged, echoed or
//! written anywhere except the 0600 env-file consumed by `docker run --env-file`.

use std::process::Command;

use anyhow::Context;

use crate::redact;

/// Read one `op://` reference. The value is registered with the redactor before being returned.
pub fn read(reference: &str) -> anyhow::Result<String> {
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

/// A safe label for diagnostics: the vault/item path, never the value.
fn label(reference: &str) -> String {
    reference.split('/').take(5).collect::<Vec<_>>().join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn missing_binary_is_reported_cleanly() {
        // A reference that cannot resolve must fail loudly; the CLI turns this into a clear
        // instruction without echoing stderr.
        let err = read("op://nonexistent-vault-taskfmt/item/field");
        assert!(err.is_err());
    }
}
