//! Local process and Git inspection needed by verification.

pub(crate) mod git;
pub(crate) mod ignore;

use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Default)]
pub(crate) struct Captured {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Captured {
    #[cfg(test)]
    pub(crate) fn ok(&self) -> bool {
        self.status == 0
    }
}

pub(crate) fn capture(command: &mut Command) -> std::io::Result<Captured> {
    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    Ok(Captured {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub(crate) fn write_file(path: &std::path::Path, contents: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(path)?;
    file.write_all(contents.as_bytes())?;
    Ok(())
}
