//! `gh` wrapper: private experiment repositories.

use std::process::Command;

use super::check;

/// Create a private repo `<owner>/<name>` and return its clone URL.
pub fn create_private(owner: &str, name: &str) -> anyhow::Result<String> {
    let mut cmd = Command::new("gh");
    cmd.args([
        "repo",
        "create",
        &format!("{owner}/{name}"),
        "--private",
        "--disable-issues",
    ]);
    let captured = check(&mut cmd, &format!("gh repo create {owner}/{name}"))?;
    let url = captured.stdout.trim().to_string();
    if url.is_empty() {
        Ok(format!("https://github.com/{owner}/{name}.git"))
    } else {
        Ok(url)
    }
}

/// Delete a repo. `gh repo delete` needs the `delete_repo` scope.
pub fn delete(owner: &str, name: &str) -> anyhow::Result<()> {
    let mut cmd = Command::new("gh");
    cmd.args(["repo", "delete", &format!("{owner}/{name}"), "--yes"]);
    check(&mut cmd, &format!("gh repo delete {owner}/{name}"))?;
    Ok(())
}

/// Does the repo exist (as seen by the authenticated account)?
pub fn exists(owner: &str, name: &str) -> bool {
    Command::new("gh")
        .args(["repo", "view", &format!("{owner}/{name}"), "--json", "name"])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Authenticated login name (`gh api user -q .login`).
pub fn login() -> anyhow::Result<String> {
    let mut cmd = Command::new("gh");
    cmd.args(["api", "user", "-q", ".login"]);
    Ok(check(&mut cmd, "gh api user")?.stdout.trim().to_string())
}
