//! `taskfmt promote <RUN>` — push a gated workspace. Refuses unless the manifest records a PASS
//! gate and the workspace HEAD is the gated HEAD.

use std::path::Path;

use anyhow::{Context, bail};

use crate::cmds::Ctx;
use crate::ops::git;
use crate::redact;
use crate::runstate::Manifest;

pub fn run(ctx: &Ctx, run_id: &str, yes: bool) -> anyhow::Result<i32> {
    let resolved = ctx.load()?;
    let run_dir = resolved.run_dir(run_id);
    promote_run(ctx, &run_dir, yes)?;
    Ok(0)
}

/// Promote one run. Any refusal happens before a push is attempted.
pub fn promote_run(ctx: &Ctx, run_dir: &Path, yes: bool) -> anyhow::Result<()> {
    let workspace = run_dir.join("workspace");
    if !workspace.is_dir() {
        bail!("no workspace at {}", workspace.display());
    }
    let mut manifest = Manifest::load(run_dir).context("reading the run manifest")?;

    // structural refusal: the manifest must already hold a PASS verdict
    let Some(gate) = manifest.gate.as_ref() else {
        bail!(
            "gate has not been run for {} — refusing to push (run `taskfmt gate` first)",
            manifest.run
        );
    };
    if !gate.passed() {
        bail!(
            "gate is {} for {} (last line {:?}) — never pushing a failed gate",
            gate.verdict,
            manifest.run,
            gate.last_line
        );
    }
    let head = git::head(&workspace)?;
    if head != gate.head {
        bail!(
            "workspace HEAD {head} differs from the gated HEAD {} — re-run `taskfmt gate` before promoting",
            gate.head
        );
    }
    if let Some(pushed) = manifest.result_sha.as_ref() {
        bail!("{} was already promoted ({pushed})", manifest.run);
    }

    let plan = vec![
        format!("git -C {} add -A", workspace.display()),
        format!(
            "git commit -s -m \"{}: {}\"",
            manifest.task,
            title_of(run_dir)
        ),
        format!("git push origin main -> {}", manifest.repo_url),
    ];
    let interaction =
        crate::interactive::Interaction::new(ctx.interaction.auto, ctx.interaction.auto || yes);
    interaction.confirm(&format!("promote {}", manifest.run), &plan)?;

    git::add_all(&workspace)?;
    if !git::status_porcelain(&workspace)?.is_empty() {
        let message = format!("{}: {}", manifest.task, title_of(run_dir));
        git::commit(&workspace, &message, true, false)?;
    }
    let result_sha = git::head(&workspace)?;
    git::push(&workspace, "origin", "main").context("git push origin main")?;
    manifest.result_sha = Some(result_sha.clone());
    manifest.save(run_dir)?;
    redact::emit(&format!(
        "PROMOTE {} {} -> {}",
        manifest.run, result_sha, manifest.repo_url
    ));
    Ok(())
}

/// `<TASK>: <title>` from the trusted snapshot's README.
pub fn title_of(run_dir: &Path) -> String {
    let readme = run_dir.join("task-snapshot/README.md");
    std::fs::read_to_string(&readme)
        .ok()
        .and_then(|text| crate::taskfile::TaskFile::parse(text, &readme).ok())
        .map(|task| task.frontmatter.title)
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "task".to_string())
}
