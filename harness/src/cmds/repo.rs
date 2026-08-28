//! `taskfmt repo create|delete` — private GitHub repositories for experiments.

use crate::cmds::Ctx;
use crate::config::timestamp_compact;
use crate::ops::{gh, git};
use crate::redact;
use crate::runstate::RepoRecord;

/// Create a private repo and bootstrap it (empty signed commit on main). Prints the URL.
pub fn create(ctx: &Ctx, name: Option<&str>) -> anyhow::Result<i32> {
    let resolved = ctx.load()?;
    let name = match name {
        Some(name) => name.to_string(),
        None => format!(
            "{}-{}",
            resolved.cfg.github.repo_prefix,
            timestamp_compact()
        ),
    };
    let full = format!("{}/{}", resolved.cfg.github.owner, name);

    let plan = vec![
        format!("gh repo create {full} --private"),
        "clone it, make an empty DCO-signed bootstrap commit on main, push".to_string(),
    ];
    ctx.interaction.confirm(&format!("create {full}"), &plan)?;

    let url = gh::create_private(&resolved.cfg.github.owner, &name)?;
    bootstrap(&url)?;
    RepoRecord {
        name: name.clone(),
        url: url.clone(),
        created: crate::config::timestamp_rfc3339(),
    }
    .save(&resolved.runs_dir())?;
    redact::emit(&format!("created {full}"));
    redact::emit(&url);
    Ok(0)
}

/// Delete a previously created repo (default: the most recent record).
pub fn delete(ctx: &Ctx, name: Option<&str>, yes: bool) -> anyhow::Result<i32> {
    let resolved = ctx.load()?;
    let name = match name {
        Some(name) => name.to_string(),
        None => RepoRecord::load_all(&resolved.runs_dir())?
            .last()
            .map(|record| record.name.clone())
            .ok_or_else(|| anyhow::anyhow!("no recorded repo and no --name given"))?,
    };
    let full = format!("{}/{}", resolved.cfg.github.owner, name);
    let interaction =
        crate::interactive::Interaction::new(ctx.interaction.auto, ctx.interaction.auto || yes);
    interaction.confirm(
        &format!("delete {full}"),
        &["gh repo delete <owner>/<name> --yes (irreversible)".to_string()],
    )?;
    gh::delete(&resolved.cfg.github.owner, &name)?;
    RepoRecord::remove(&resolved.runs_dir(), &name)?;
    redact::emit(&format!("deleted {full}"));
    Ok(0)
}

/// Make the empty DCO-signed bootstrap commit and push `main`. A freshly created GitHub repo has
/// no branches at all, so there is nothing to clone yet: init locally, point `origin` at the new
/// repo, commit `--allow-empty`, push with `-u`.
pub fn bootstrap(url: &str) -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let dir = tmp.path().join("repo");
    std::fs::create_dir_all(&dir)?;
    git::init(&dir)?;
    git::remote_add(&dir, "origin", url)?;
    git::commit(&dir, "bootstrap", true, true)?;
    git::push_upstream(&dir, "origin", "main")?;
    Ok(())
}

/// Create (or reuse) the repo a run/experiment works against: `--repo` wins, otherwise confirm and
/// create a disposable one. Returns the clone URL.
pub fn ensure_repo(
    ctx: &Ctx,
    resolved: &crate::config::Resolved,
    provided: Option<&str>,
) -> anyhow::Result<String> {
    if let Some(url) = provided {
        return Ok(url.to_string());
    }
    let name = format!(
        "{}-{}",
        resolved.cfg.github.repo_prefix,
        timestamp_compact()
    );
    let full = format!("{}/{}", resolved.cfg.github.owner, name);
    let plan = vec![
        format!("gh repo create {full} --private (disposable experiment repo)"),
        "bootstrap main with an empty DCO-signed commit".to_string(),
        "this run's workspace is cloned from it; the trusted base commit stays local".to_string(),
    ];
    if !ctx.interaction.confirm(&format!("create {full}"), &plan)? {
        anyhow::bail!("declined to create {full}; pass --repo <url> to use an existing repository");
    }
    let url = gh::create_private(&resolved.cfg.github.owner, &name)?;
    bootstrap(&url)?;
    RepoRecord {
        name,
        url: url.clone(),
        created: crate::config::timestamp_rfc3339(),
    }
    .save(&resolved.runs_dir())?;
    Ok(url)
}
