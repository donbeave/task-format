//! `taskfmt experiment --tasks all` — run a task batch against one repo, gate each, promote only
//! on PASS. State lives in `runs_dir/<ID>/experiment.json`; stop on the first FAIL/BLOCKED.

use anyhow::{Context, bail};

use crate::cmds::Ctx;
use crate::redact;
use crate::runstate::ExperimentState;

pub fn run(
    ctx: &Ctx,
    tasks: &[String],
    repo: Option<&str>,
    agent: Option<&str>,
    resume: Option<&str>,
    kill_after: Option<u64>,
    selfcheck: bool,
) -> anyhow::Result<i32> {
    let resolved = ctx.load()?;
    let profile_name = agent
        .unwrap_or_else(|| resolved.cfg.default_profile())
        .to_string();

    // The experiment state comes first: a resume is pinned to the recorded repo, so the state must
    // be known before any repo is created. Creating first is how a resumed experiment once ended up
    // on a brand-new repo whose `main` had none of the earlier tasks' commits.
    let experiment_id = match resume {
        Some(id) => {
            let path = resolved.experiment_file(id);
            if !path.is_file() {
                bail!(
                    "no experiment state at {} — nothing to resume",
                    path.display()
                );
            }
            redact::emit(&format!("== resume experiment {id}"));
            id.to_string()
        }
        None => format!("exp-{}", crate::config::timestamp_compact()),
    };
    let state_file = resolved.experiment_file(&experiment_id);
    let existing = ExperimentState::load(&state_file)?;
    let repo_url = resolve_repo_url(existing.as_ref(), repo, |provided| {
        crate::cmds::repo::ensure_repo(ctx, &resolved, provided)
    })?;
    let mut state = existing.unwrap_or_else(|| ExperimentState::new(&experiment_id, &repo_url));
    // Before anything is dispatched, and before the confirmation: `ensure_repo` above may already
    // have minted a live repository, and a failure between there and the first save would leave it
    // with nothing naming it and `--resume` nothing to resume.
    state.save(&state_file)?;

    let selection =
        crate::selection::resolve(tasks, &resolved.tasks_dir()).context("resolving --tasks")?;
    if selection.is_empty() {
        bail!("no tasks selected — pass --tasks all, --tasks 1-3 or --tasks TASK-101");
    }
    let done: Vec<String> = state
        .tasks
        .iter()
        .filter(|task| task.pushed)
        .map(|task| task.task.clone())
        .collect();
    let pending = crate::selection::skip_completed(&selection, &done);
    if pending.len() < selection.len() {
        redact::emit(&format!(
            "== resuming: {} already pushed ({}), {} to go",
            done.len(),
            done.join(", "),
            pending.len()
        ));
    }
    if pending.is_empty() {
        redact::emit("nothing left to run — every selected task already passed");
        return Ok(0);
    }

    let plan: Vec<String> = pending
        .iter()
        .map(|task| format!("run {task} on {repo_url} as {profile_name}, gate, promote on PASS"))
        .collect();
    if ctx
        .interaction
        .confirm(&format!("experiment {experiment_id}"), &plan)?
        == crate::interactive::Decision::Declined
    {
        redact::eemit("aborted — nothing dispatched");
        return Ok(2);
    }

    let mut failed = 0usize;
    for (index, task_id) in pending.iter().enumerate() {
        redact::emit(&format!(
            "== [{}/{}] {task_id}",
            index + 1 + done.len(),
            selection.len()
        ));
        let mut outcome = match crate::cmds::run::dispatch_one(
            &resolved,
            &profile_name,
            None,
            None,
            task_id,
            &repo_url,
            Some(&experiment_id),
            selfcheck,
            // the one image reader in the crate; a dispatch compares whatever it returns
            &crate::ops::docker::DockerImageFingerprint,
        ) {
            Ok(outcome) => outcome,
            Err(err) => {
                redact::eemit(&format!("dispatch failed for {task_id}: {err:#}"));
                failed += 1;
                break;
            }
        };

        // attached: poll until terminal, gate, promote only on PASS. wait_and_gate records the
        // gate into the outcome's manifest, so the promotable check below sees the real verdict.
        let mut manifest = outcome.manifest;
        let status = match crate::cmds::run::wait_and_gate(
            &mut manifest,
            &outcome.run_dir,
            &resolved,
            kill_after,
        ) {
            Ok(status) => status,
            Err(err) => {
                redact::eemit(&format!("wait or gate failed for {task_id}: {err:#}"));
                state.record_task(crate::runstate::ExperimentTask {
                    task: task_id.clone(),
                    repo_url: repo_url.clone(),
                    base_sha: manifest.base_sha.clone(),
                    result_sha: None,
                    gate: manifest
                        .gate
                        .as_ref()
                        .map(|gate| gate.verdict.clone())
                        .unwrap_or_else(|| "none".to_string()),
                    pushed: false,
                    run_dir: outcome.run_dir.display().to_string(),
                });
                state.save(&state_file)?;
                return Ok(1);
            }
        };
        outcome.manifest = manifest;
        let gated = outcome.manifest.gate.as_ref();
        let mut entry = crate::runstate::ExperimentTask {
            task: task_id.clone(),
            repo_url: repo_url.clone(),
            base_sha: outcome.manifest.base_sha.clone(),
            result_sha: None,
            gate: gated
                .map(|gate| gate.verdict.clone())
                .unwrap_or_else(|| "none".to_string()),
            pushed: false,
            run_dir: outcome.run_dir.display().to_string(),
        };

        let promotable = crate::cmds::status::is_promotable(&status)
            && outcome
                .manifest
                .gate
                .as_ref()
                .is_some_and(|gate| gate.passed());
        if !promotable {
            redact::eemit(&format!(
                "stopping: {task_id} ended {} ({}) with gate {} — remaining tasks untouched",
                status.state,
                status
                    .terminal_reason
                    .as_deref()
                    .unwrap_or("no terminal reason"),
                entry.gate
            ));
            state.record_task(entry);
            state.save(&state_file)?;
            return Ok(1);
        }

        match crate::cmds::promote::promote_run(ctx, &outcome.run_dir, true) {
            Ok(()) => {
                let refreshed = crate::runstate::Manifest::load(&outcome.run_dir)?;
                entry.result_sha = refreshed.result_sha.clone();
                entry.pushed = refreshed.result_sha.is_some();
            }
            Err(err) => {
                redact::eemit(&format!("promote refused for {task_id}: {err:#}"));
                state.record_task(entry);
                state.save(&state_file)?;
                return Ok(1);
            }
        }
        state.record_task(entry);
        state.save(&state_file)?;
    }

    redact::emit(&format!(
        "EXPERIMENT {} pass={} fail={} (state: {})",
        experiment_id,
        state.passed_tasks().len(),
        failed,
        state_file.display()
    ));
    Ok(if failed == 0 { 0 } else { 1 })
}

/// The repository an experiment run works against. Recorded state pins it (`resume_repo_url`);
/// without state, the `create` fallback decides — in `run` that is `repo::ensure_repo`, which uses
/// `--repo` when given and otherwise confirms and mints a disposable repo.
pub fn resolve_repo_url(
    state: Option<&ExperimentState>,
    repo_arg: Option<&str>,
    create: impl FnOnce(Option<&str>) -> anyhow::Result<String>,
) -> anyhow::Result<String> {
    match state {
        Some(state) => resume_repo_url(state, repo_arg),
        None => create(repo_arg),
    }
}

/// The repository a resumed experiment continues on: the recorded `repo_url`, always. Each task
/// builds on the previous task's pushed chain, so a resume on any other repo — let alone a freshly
/// created empty one — would fork the experiment. A `--repo` that matches the record is accepted;
/// one that does not is an error naming both.
pub fn resume_repo_url(state: &ExperimentState, repo_arg: Option<&str>) -> anyhow::Result<String> {
    match repo_arg {
        None => Ok(state.repo_url.clone()),
        Some(arg) if arg == state.repo_url => Ok(state.repo_url.clone()),
        Some(arg) => bail!(
            "--repo {arg} does not match experiment {} ({reco}); a resume always continues on the \
             recorded repo — start a new experiment to use a different one",
            state.id,
            reco = state.repo_url,
        ),
    }
}
