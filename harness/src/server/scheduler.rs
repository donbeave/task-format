use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, ensure};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::task::JoinSet;

use super::{ApiError, AppState, Inner, adapter, atomic_json, eligibility};
use crate::monitor::{Catalog, ProgressSummary, Scope, Status};
use crate::progress::ProgressFile;
use crate::taskfile::TaskFile;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    pub id: String,
    pub task_ids: Vec<String>,
    pub state: String,
    pub started: String,
    pub finished: Option<String>,
    pub tasks: BTreeMap<String, TaskRun>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskRun {
    pub artifact_id: String,
    pub state: String,
    pub failure: Option<String>,
}

fn save(root: &Path, execution: &Execution) -> anyhow::Result<()> {
    atomic_json::write(&root.join(format!("{}.json", execution.id)), execution)
}

pub(super) fn read_executions(root: &Path) -> anyhow::Result<Vec<Execution>> {
    let mut executions = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        ensure!(!entry.file_type()?.is_symlink(), "symlink in run root");
        if entry.path().extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let execution: Execution = serde_json::from_slice(&std::fs::read(entry.path())?)?;
        uuid::Uuid::parse_str(&execution.id)?;
        ensure!(
            entry.file_name().to_str() == Some(&format!("{}.json", execution.id)),
            "execution filename mismatch"
        );
        for id in &execution.task_ids {
            crate::monitor::validate_id(id)?;
        }
        for (id, task) in &execution.tasks {
            ensure!(
                execution.task_ids.contains(id),
                "journal task outside scope"
            );
            uuid::Uuid::parse_str(&task.artifact_id)?;
        }
        executions.push(execution);
    }
    executions.sort_by(|a, b| a.started.cmp(&b.started));
    Ok(executions)
}

pub(super) fn latest_run(inner: &Inner, id: &str) -> Value {
    inner.executions.iter().rev().find_map(|execution|execution.tasks.get(id).map(|run|json!({
        "execution_id":execution.id,"state":run.state,"failure":run.failure,"started":execution.started,"finished":execution.finished
    }))).unwrap_or(Value::Null)
}

pub(super) fn enrich(catalog: &mut Catalog, inner: &Inner) -> anyhow::Result<()> {
    for task in catalog.tasks.values_mut() {
        let run = inner
            .executions
            .iter()
            .rev()
            .find_map(|execution| execution.tasks.get(&task.id));
        let manifest = if let Some(run) = run {
            let root = inner.config.runs_root.join(&run.artifact_id);
            if root.exists() {
                match adapter::find_manifest(&root) {
                    Ok(manifest) => manifest,
                    Err(error) if task.metadata.status == Status::Done => return Err(error),
                    Err(_) => {
                        task.progress.current_failure=Some("Run progress is temporarily unavailable while artifacts are being updated.".into());
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };
        if task.metadata.status == Status::Done {
            let manifest = manifest
                .as_ref()
                .context("done task lacks authoritative run evidence")?;
            ensure!(
                adapter::verified(manifest),
                "done task lacks a verified gate"
            );
            ensure!(
                manifest.task == task.contract_id,
                "task manifest identity mismatch"
            );
            // The domain checks immutable task and verifier identities as well as gate authority.
            crate::monitor::verify_task_evidence(task, manifest)?;
        }
        if let Some(manifest) = manifest {
            ensure!(
                manifest.task == task.contract_id,
                "run belongs to a different task contract"
            );
            let parsed = TaskFile::parse(task.readme.clone(), &task.path.join("README.md"))?;
            let run_path = std::path::PathBuf::from(&manifest.run_dir);
            let progress_path = run_path.join("progress/progress.md");
            let mut progress_error = false;
            let progress = if progress_path.exists() {
                crate::monitor::safe_path(&inner.config.runs_root, &progress_path)?;
                match ProgressFile::load(&progress_path, &parsed) {
                    Ok(progress) => Some(progress),
                    Err(_) => {
                        progress_error = true;
                        None
                    }
                }
            } else {
                None
            };
            task.progress =
                ProgressSummary::from_task(&parsed, task.metadata.status, progress.as_ref());
            if progress_error {
                task.progress.current_failure=Some("Checklist progress is temporarily unavailable or malformed; execution authority is unchanged.".into());
            }
            if !manifest.status_state.is_empty() {
                task.progress.run_state = Some(manifest.status_state);
            }
        }
        if let Some(run) = run
            && run.failure.is_some()
        {
            task.progress.current_failure.clone_from(&run.failure);
        }
    }
    Ok(())
}

fn validate_authority(catalog: &Catalog, inner: &Inner) -> anyhow::Result<()> {
    for task in catalog
        .tasks
        .values()
        .filter(|task| task.metadata.status == Status::Done)
    {
        let run = inner
            .executions
            .iter()
            .rev()
            .find_map(|execution| execution.tasks.get(&task.id))
            .context("done task lacks durable execution")?;
        let manifest = adapter::find_manifest(&inner.config.runs_root.join(&run.artifact_id))?
            .context("done task lacks run evidence")?;
        crate::monitor::verify_task_evidence(task, &manifest)?;
    }
    Ok(())
}

pub(super) async fn start(state: AppState, scope: Scope) -> Result<Execution, ApiError> {
    state.refresh_availability().await?;
    if *state.cancel.borrow() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "shutting_down",
            "Server is shutting down",
        ));
    }
    let outcome = state
        .access(move |inner| {
            let catalog = Catalog::load(&inner.config.tasks_root)?;
            validate_authority(&catalog, inner)?;
            if catalog.scope_tasks(&scope).is_err() {
                return Ok(Err(ApiError::new(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "Requested scope does not exist",
                )));
            }
            let eligibility = eligibility(inner, &catalog, &scope);
            if !eligibility.allowed {
                return Ok(Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "not_eligible",
                    eligibility.reason.unwrap_or_default(),
                )));
            }
            let task_ids = catalog.validate_scope(&scope)?;
            let execution = Execution {
                id: uuid::Uuid::new_v4().to_string(),
                task_ids,
                state: "running".into(),
                started: chrono::Utc::now().to_rfc3339(),
                finished: None,
                tasks: BTreeMap::new(),
            };
            save(&inner.config.runs_root, &execution)?;
            inner
                .reservations
                .extend(execution.task_ids.iter().cloned());
            inner.executions.push(execution.clone());
            Ok(Ok(execution))
        })
        .await??;
    let owned_state = state.clone();
    let execution = outcome.clone();
    let mut workers = state.workers.lock().await;
    while let Some(result) = workers.try_join_next() {
        if let Err(error) = result {
            eprintln!("monitor scheduler failed: {error}");
        }
    }
    workers.spawn(async move {
        if let Err(error) = schedule(&owned_state, execution).await {
            eprintln!("monitor scheduler stopped: {error:?}");
        }
    });
    Ok(outcome)
}

async fn schedule(state: &AppState, mut execution: Execution) -> Result<(), ApiError> {
    let mut waiting: BTreeSet<String> = execution.task_ids.iter().cloned().collect();
    let mut running = JoinSet::new();
    let mut failed = false;
    let scheduling = async {
    loop {
        if !*state.cancel.borrow() {
            let ids: Vec<_> = waiting.iter().cloned().collect();
            let ready = state.access(move |inner| { let catalog=Catalog::load(&inner.config.tasks_root)?; validate_authority(&catalog,inner)?; Ok(catalog.ready_tasks(&ids)) }).await?;
            for id in ready {
                let Ok(permit) = state.slots.clone().try_acquire_owned() else { break; };
                waiting.remove(&id);
                let task_run = TaskRun {artifact_id:uuid::Uuid::new_v4().to_string(),state:"running".into(),failure:None};
                execution.tasks.insert(id.clone(),task_run.clone());
                let record = execution.clone();
                let task_id = id.clone();
                let (config,package,artifacts) = state.access(move |inner| {
                    let mut catalog=Catalog::load(&inner.config.tasks_root)?;
                    let artifacts=inner.config.runs_root.join(&task_run.artifact_id);
                    std::fs::create_dir(&artifacts)?;
                    save(&inner.config.runs_root,&record)?;
                    replace(inner,&record);
                    catalog.start(&task_id)?;
                    let task=catalog.tasks.get(&task_id).context("task disappeared")?;
                    Ok((inner.config.adapter.clone().context("execution unavailable")?,task.path.clone(),artifacts))
                }).await?;
                let cancel=state.cancel.subscribe();
                running.spawn(async move {
                    let result=config.execute(package,artifacts.clone(),cancel).await;
                    let cleanup=adapter::cleanup(&artifacts).await;
                    drop(permit);
                    (id,result,cleanup)
                });
            }
        }
        if running.is_empty() {
            if waiting.is_empty() || *state.cancel.borrow() { break; }
            // Other disjoint scopes may own the global slots. Wait only while an actual
            // tracked execution can free one; blocked prerequisites terminate this scope.
            if state.slots.available_permits() == 0 {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
            failed=true;
            break;
        }
        let Some(result)=running.join_next().await else { break; };
        let (id,result,cleanup)=result.map_err(|error|ApiError::internal(error.into()))?;
        let mut verified=false;
        let clean=cleanup.is_ok();
        if let Ok(Some(manifest))=&result { verified=adapter::verified(manifest); }
        let run=execution.tasks.get_mut(&id).expect("tracked task has a journal");
        run.state=if !clean {"cleanup_required"} else if verified {"done"} else {"failed"}.into();
        if !verified || !clean { run.failure=Some(if clean {"Task execution did not produce successful authoritative verification."}else{"Executor cleanup is unconfirmed; restart after restoring Docker availability."}.into()); failed=true; }
        let record=execution.clone();
        state.access(move |inner| {
            let mut catalog=Catalog::load(&inner.config.tasks_root)?;
            if verified {
                let manifest=result?.context("missing manifest")?;
                let task=catalog.tasks.get(&id).context("task missing")?;
                crate::monitor::verify_task_evidence(task,&manifest)?;
            }
            if clean { catalog.finish(&id,verified)?; }
            save(&inner.config.runs_root,&record)?;
            replace(inner,&record);
            Ok(())
        }).await?;
    }
    Ok::<(),ApiError>(())
    }.await;
    if let Err(error) = scheduling {
        // A catalog/journal error invalidates global scheduling assumptions. Signal every
        // owned adapter, then join them: dropping JoinSet would skip asynchronous cleanup.
        state.cancel.send_replace(true);
        while let Some(result) = running.join_next().await {
            match result {
                Ok((id, _, cleanup)) => {
                    if let Some(run) = execution.tasks.get_mut(&id) {
                        run.state = if cleanup.is_ok() {
                            "interrupted"
                        } else {
                            "cleanup_required"
                        }
                        .into();
                        run.failure = Some(
                            "Scheduling failed; execution stopped. Restart to reconcile metadata."
                                .into(),
                        );
                    }
                }
                Err(join_error) => eprintln!("monitor executor join failed: {join_error}"),
            }
        }
        execution.state = "failed".into();
        execution.finished = Some(chrono::Utc::now().to_rfc3339());
        let persist = state
            .access(move |inner| {
                save(&inner.config.runs_root, &execution)?;
                replace(inner, &execution);
                Ok(())
            })
            .await;
        if let Err(persist_error) = persist {
            eprintln!("cannot persist scheduler failure: {persist_error:?}");
        }
        return Err(error);
    }
    execution.state = if *state.cancel.borrow() {
        "cancelled"
    } else if failed {
        "failed"
    } else {
        "done"
    }
    .into();
    execution.finished = Some(chrono::Utc::now().to_rfc3339());
    state
        .access(move |inner| {
            save(&inner.config.runs_root, &execution)?;
            for id in &execution.task_ids {
                inner.reservations.remove(id);
            }
            replace(inner, &execution);
            Ok(())
        })
        .await
}

fn replace(inner: &mut Inner, execution: &Execution) {
    if let Some(current) = inner
        .executions
        .iter_mut()
        .find(|item| item.id == execution.id)
    {
        *current = execution.clone();
    }
}

pub(super) async fn recover(state: &AppState) -> anyhow::Result<()> {
    let roots = state
        .access(|inner| {
            Ok(inner
                .executions
                .iter()
                .flat_map(|execution| {
                    execution
                        .tasks
                        .values()
                        .filter(|run| matches!(run.state.as_str(), "running" | "cleanup_required"))
                        .map(|run| inner.config.runs_root.join(&run.artifact_id))
                })
                .collect::<Vec<_>>())
        })
        .await
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    for root in roots {
        let _lease = adapter::await_supervisor(&root).await?;
        adapter::cleanup(&root).await?;
    }
    state.access(|inner| {
        let mut catalog=Catalog::load(&inner.config.tasks_root)?;
        let ids:Vec<_>=catalog.tasks.values().filter(|task|task.metadata.status==Status::InProgress).map(|task|task.id.clone()).collect();
        for id in ids {
            let run=inner.executions.iter().rev().find_map(|execution|execution.tasks.get(&id));
            let manifest=run.map(|run|adapter::find_manifest(&inner.config.runs_root.join(&run.artifact_id))).transpose()?.flatten();
            let verified=manifest.as_ref().is_some_and(|manifest|adapter::verified(manifest)&&catalog.tasks.get(&id).is_some_and(|task|crate::monitor::verify_task_evidence(task,manifest).is_ok()));
            catalog.recover(&id,verified)?;
        }
        for execution in &mut inner.executions {
            if execution.state=="running" || execution.tasks.values().any(|run|run.state=="cleanup_required") {
                execution.state="interrupted".into(); execution.finished=Some(chrono::Utc::now().to_rfc3339());
                for (id,run) in &mut execution.tasks { if matches!(run.state.as_str(),"running"|"cleanup_required") {
                    if catalog.tasks.get(id).is_some_and(|task|task.metadata.status==Status::Done) {
                        run.state="done".into(); run.failure=None;
                    } else {
                        run.state="interrupted".into(); run.failure=Some("Backend restarted; executor stopped and authoritative run evidence reconciled.".into());
                    }
                } }
                save(&inner.config.runs_root,execution)?;
            }
        }
        enrich(&mut catalog,inner)?;
        Ok(())
    }).await.map_err(|error|anyhow::anyhow!("{error:?}"))
}
