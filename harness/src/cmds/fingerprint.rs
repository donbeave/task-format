//! `taskfmt fingerprint` — what gate is this, and what gate is in that image?
//!
//! Three answers from one algorithm (`crate::fingerprint`): the value compiled into this binary,
//! the value a crate directory on disk would produce, and the value an image reports when its own
//! `/usr/local/bin/taskfmt` is executed. The first is what `taskfmt run` compares at dispatch; the
//! second is inspection only and feeds no decision; the third is the image side of that comparison.

use std::path::Path;

use anyhow::Context;

use crate::ops::docker::{DockerImageFingerprint, ImageFingerprint};

pub fn run(path: Option<&Path>, image: Option<&str>) -> anyhow::Result<i32> {
    let value = match (path, image) {
        (Some(dir), _) => crate::fingerprint::fingerprint(dir)
            .with_context(|| format!("cannot fingerprint {}", dir.display()))?,
        (None, Some(image)) => DockerImageFingerprint.image_fingerprint(image)?,
        (None, None) => crate::HARNESS_FINGERPRINT.to_string(),
    };
    println!("{value}");
    Ok(0)
}

/// The refusal, as a pure function of the two values: `Ok` when the host and the image were built
/// from the same sources, an error naming **both** values and the remedies when they were not.
///
/// Both remedies are named because either side can be the stale one. `taskfmt build-images`
/// rebuilds the image from source and is the fix when the image lags; it repairs nothing when the
/// host binary is the stale side, which is what an operator who edited the crate and did not
/// reinstall is looking at.
pub fn compare(host: &str, image: &str, image_value: &str) -> anyhow::Result<()> {
    if host == image_value {
        return Ok(());
    }
    anyhow::bail!(
        "the gate baked into {image} is a different build from this binary: host {host}, image \
         {image_value}. Rebuild the image with `taskfmt build-images`, or reinstall the host \
         binary with `cargo install --path harness` if the host is the stale side."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_compiled_in_value_is_the_shape_the_comparison_expects() {
        assert_eq!(crate::HARNESS_FINGERPRINT.len(), 64);
        assert!(
            crate::HARNESS_FINGERPRINT
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
            "{}",
            crate::HARNESS_FINGERPRINT
        );
        compare(
            crate::HARNESS_FINGERPRINT,
            "harness-claude:latest",
            crate::HARNESS_FINGERPRINT,
        )
        .unwrap();
    }
}
