//! Filesystem catalog inspection and a thin client for the monitor's execution authority.

use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::cli::{CatalogOptions, GroupCmd, MonitorRunOptions, ProjectCmd};
use crate::monitor::{Catalog, Scope};
use crate::server::Execution;

use super::Ctx;

const SCHEMA: &str = "taskfmt/project-cli/v1";
const RESPONSE_LIMIT: u64 = 8 * 1024 * 1024;

#[derive(Debug, Serialize)]
struct CliError {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    monitor_code: Option<String>,
}

impl CliError {
    fn new(code: &'static str, message: impl ToString) -> Self {
        Self {
            code,
            message: message.to_string(),
            http_status: None,
            monitor_code: None,
        }
    }
}

struct Output {
    data: Value,
    text: String,
    exit: i32,
}

fn render(result: Result<Output, CliError>, as_json: bool) -> anyhow::Result<i32> {
    match result {
        Ok(output) => {
            if as_json {
                crate::redact::emit(&serde_json::to_string(
                    &json!({"schema":SCHEMA,"data":output.data}),
                )?);
            } else {
                crate::redact::emit(&output.text);
            }
            Ok(output.exit)
        }
        Err(error) => {
            if as_json {
                crate::redact::emit(&serde_json::to_string(
                    &json!({"schema":SCHEMA,"error":error}),
                )?);
            } else {
                crate::redact::eemit(&format!("taskfmt: {}: {}", error.code, error.message));
            }
            Ok(1)
        }
    }
}

/// Dispatch a first-class project command without altering flat task CLI semantics.
pub fn project(ctx: &Ctx, command: &ProjectCmd) -> anyhow::Result<i32> {
    match command {
        ProjectCmd::List { options } => {
            render(read(ctx, options, None, false, false), options.json)
        }
        ProjectCmd::Show { project, options } => render(
            project_scope(project).and_then(|scope| read(ctx, options, Some(scope), false, false)),
            options.json,
        ),
        ProjectCmd::Lint { project, options } => render(
            project_scope(project).and_then(|scope| read(ctx, options, Some(scope), false, true)),
            options.json,
        ),
        ProjectCmd::Run { project, options } => render(
            project_scope(project).and_then(|scope| execute(ctx, scope, options)),
            options.json,
        ),
    }
}

/// Dispatch a first-class group command. Group references are PROJECT/GROUP.
pub fn group(ctx: &Ctx, command: &GroupCmd) -> anyhow::Result<i32> {
    match command {
        GroupCmd::List { project, options } => render(
            project_scope(project).and_then(|scope| read(ctx, options, Some(scope), true, false)),
            options.json,
        ),
        GroupCmd::Show { group, options } => render(
            group_scope(group).and_then(|scope| read(ctx, options, Some(scope), false, false)),
            options.json,
        ),
        GroupCmd::Lint { group, options } => render(
            group_scope(group).and_then(|scope| read(ctx, options, Some(scope), false, true)),
            options.json,
        ),
        GroupCmd::Run { group, options } => render(
            group_scope(group).and_then(|scope| execute(ctx, scope, options)),
            options.json,
        ),
    }
}

fn project_scope(code: &str) -> Result<Scope, CliError> {
    crate::monitor::validate_code(code)
        .map_err(|error| CliError::new("invalid_identifier", error))?;
    Ok(Scope::Project(code.to_owned()))
}

fn group_scope(id: &str) -> Result<Scope, CliError> {
    let (project, group) = id
        .split_once('/')
        .ok_or_else(|| CliError::new("invalid_identifier", "group must be PROJECT/GROUP"))?;
    crate::monitor::validate_code(project)
        .and_then(|()| crate::monitor::validate_code(group))
        .map_err(|error| CliError::new("invalid_identifier", error))?;
    Ok(Scope::Group {
        project: project.into(),
        group: group.into(),
    })
}

fn read(
    ctx: &Ctx,
    options: &CatalogOptions,
    scope: Option<Scope>,
    groups_only: bool,
    lint: bool,
) -> Result<Output, CliError> {
    let root = match &options.projects_root {
        Some(root) => root.clone(),
        None => ctx
            .load()
            .map_err(|error| CliError::new("configuration_invalid", error))?
            .projects_dir(),
    };
    let catalog = Catalog::load(&root).map_err(|error| CliError::new("catalog_invalid", error))?;
    let ids = match &scope {
        Some(scope) => catalog
            .scope_tasks(scope)
            .map_err(|error| CliError::new("not_found", error))?,
        None => catalog.tasks.keys().cloned().collect(),
    };
    if lint {
        let reports: Vec<_> = ids
            .iter()
            .map(|id| crate::lint::lint_path(&catalog.tasks[id].path))
            .collect();
        let passed = reports.iter().all(crate::lint::LintReport::passed);
        let mut text = String::new();
        for report in &reports {
            text.push_str(&format!(
                "PACKAGE {}\n{}",
                report.target.display(),
                report.render()
            ));
        }
        text.push_str(&format!(
            "{}: {}",
            if passed { "PASS" } else { "FAIL" },
            quantity(reports.len(), "task package")
        ));
        return Ok(Output {
            data: json!({"authority":"filesystem_contracts","passed":passed,"reports":reports}),
            text,
            exit: i32::from(!passed),
        });
    }
    let mut items = Vec::new();
    let mut text = String::from(
        "Filesystem metadata; live run progress and verification evidence are not loaded.\n",
    );
    match scope {
        None => {
            for project in catalog.projects.values() {
                let task_ids = catalog
                    .scope_tasks(&Scope::Project(project.code.clone()))
                    .map_err(|error| CliError::new("catalog_invalid", error))?;
                items.push(json!({"project":project,"counts":counts(&catalog,&task_ids)}));
                text.push_str(&format!(
                    "{} — {} ({}, {})\n",
                    project.code,
                    project.name,
                    quantity(project.groups.len(), "group"),
                    quantity(task_ids.len(), "task")
                ));
            }
        }
        Some(Scope::Project(code)) => {
            let project = &catalog.projects[&code];
            if !groups_only {
                text.push_str(&format!(
                    "{} — {}\n{}\n",
                    project.code, project.name, project.readme
                ));
            }
            for id in &project.groups {
                let group = &catalog.groups[id];
                items.push(json!({"group":group,"counts":counts(&catalog,&group.tasks)}));
                text.push_str(&format!(
                    "{} — {} ({})\n",
                    group.id,
                    group.name,
                    quantity(group.tasks.len(), "task")
                ));
            }
            if !groups_only {
                let tasks = task_metadata(&catalog, &ids, &mut text);
                return Ok(Output {
                    data: json!({"authority":"filesystem_metadata","project":project,"groups":items,"tasks":tasks,"counts":counts(&catalog,&ids)}),
                    text,
                    exit: 0,
                });
            }
        }
        Some(Scope::Group { project, group }) => {
            let record = &catalog.groups[&format!("{project}/{group}")];
            text.push_str(&format!(
                "{} — {}\n{}\n",
                record.id, record.name, record.readme
            ));
            let tasks = task_metadata(&catalog, &ids, &mut text);
            return Ok(Output {
                data: json!({"authority":"filesystem_metadata","group":record,"tasks":tasks,"counts":counts(&catalog,&ids)}),
                text,
                exit: 0,
            });
        }
        Some(Scope::Task(_)) => {
            return Err(CliError::new(
                "invalid_identifier",
                "expected project or group",
            ));
        }
    }
    if items.is_empty() {
        text.push_str("No entries.\n");
    }
    Ok(Output {
        data: json!({"authority":"filesystem_metadata","items":items}),
        text,
        exit: 0,
    })
}

fn counts(catalog: &Catalog, ids: &[String]) -> Value {
    let summary = catalog.aggregate(ids);
    json!({"task_count":summary.task_count,"draft":summary.draft,"pending":summary.pending,"in_progress":summary.in_progress,"done":summary.done,"blocked":summary.blocked})
}

fn quantity(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
}

fn task_metadata(catalog: &Catalog, ids: &[String], text: &mut String) -> Vec<Value> {
    ids.iter()
        .map(|id| {
            let task = &catalog.tasks[id];
            text.push_str(&format!(
                "{} {:?} — {}\n",
                task.id, task.metadata.status, task.title
            ));
            json!({"id":task.id,"title":task.title,"metadata":task.metadata})
        })
        .collect()
}

fn monitor_client(address: &str) -> Result<(Client, reqwest::Url), CliError> {
    let url = reqwest::Url::parse(address).map_err(|_| {
        CliError::new(
            "invalid_monitor_url",
            "monitor URL must be a loopback HTTP origin",
        )
    })?;
    let host = url.host_str().unwrap_or_default();
    let ip = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<IpAddr>()
        .ok();
    if url.scheme() != "http"
        || !(host == "localhost" || ip.is_some_and(|ip| ip.is_loopback()))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err(CliError::new(
            "invalid_monitor_url",
            "monitor URL must be a loopback HTTP origin without credentials, path, query, or fragment",
        ));
    }
    let mut builder = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(10));
    if host == "localhost" {
        builder = builder.resolve(
            "localhost",
            SocketAddr::new(
                Ipv4Addr::LOCALHOST.into(),
                url.port_or_known_default().unwrap_or(80),
            ),
        );
    }
    let client = builder
        .build()
        .map_err(|error| CliError::new("request_failed", error))?;
    Ok((client, url))
}

#[derive(Deserialize)]
struct MonitorError {
    error: MonitorErrorBody,
}
#[derive(Deserialize)]
struct MonitorErrorBody {
    code: String,
    message: String,
}

fn decode<T: DeserializeOwned>(response: Response, expected_status: u16) -> Result<T, CliError> {
    let status = response.status().as_u16();
    let mut bytes = Vec::new();
    response
        .take(RESPONSE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| CliError::new("request_failed", error))?;
    if bytes.len() as u64 > RESPONSE_LIMIT {
        return Err(CliError::new(
            "invalid_response",
            "monitor response exceeds 8 MiB",
        ));
    }
    if status != expected_status {
        let parsed = serde_json::from_slice::<MonitorError>(&bytes).ok();
        return Err(CliError {
            code: "monitor_rejected",
            message: parsed.as_ref().map_or_else(
                || format!("monitor returned HTTP {status}"),
                |error| error.error.message.clone(),
            ),
            http_status: Some(status),
            monitor_code: parsed.map(|error| error.error.code),
        });
    }
    serde_json::from_slice(&bytes).map_err(|error| CliError::new("invalid_response", error))
}

fn execute(ctx: &Ctx, scope: Scope, options: &MonitorRunOptions) -> Result<Output, CliError> {
    let (client, mut url) = monitor_client(&options.monitor_url)?;
    let (path, label) = match scope {
        Scope::Project(code) => (
            format!("/api/run/project/{code}"),
            format!("project {code}"),
        ),
        Scope::Group { project, group } => (
            format!("/api/run/group/{project}/{group}"),
            format!("group {project}/{group}"),
        ),
        Scope::Task(_) => {
            return Err(CliError::new(
                "invalid_identifier",
                "expected project or group",
            ));
        }
    };
    ctx.interaction
        .confirm(
            &format!("run {label}"),
            &[format!(
                "Request execution from {} using its configured repository and agent",
                options.monitor_url
            )],
        )
        .and_then(|answer| answer.or_decline("running scope"))
        .map_err(|error| CliError::new("consent_required", error))?;
    url.set_path(&path);
    let response = client
        .post(url.clone())
        .header("x-task-monitor", "1")
        .send()
        .map_err(|error| CliError::new("request_failed", error))?;
    let mut execution: Execution = decode(response, 202)?;
    let id = execution.id.clone();
    if uuid::Uuid::parse_str(&id).is_err() {
        return Err(CliError::new(
            "invalid_response",
            "monitor returned an invalid execution ID",
        ));
    }
    url.set_path("/api/executions");
    while options.wait && execution.state == "running" {
        std::thread::sleep(Duration::from_millis(500));
        let response = client.get(url.clone()).send().map_err(|error| {
            CliError::new(
                "request_failed",
                format!("execution {id} remains owned by monitor; polling failed: {error}"),
            )
        })?;
        let journals: Vec<Execution> = decode(response, 200)?;
        execution = journals
            .into_iter()
            .find(|execution| execution.id == id)
            .ok_or_else(|| {
                CliError::new(
                    "execution_missing",
                    format!("monitor no longer reports execution {id}"),
                )
            })?;
    }
    if !matches!(
        execution.state.as_str(),
        "running" | "done" | "failed" | "cancelled" | "interrupted"
    ) {
        return Err(CliError::new(
            "invalid_response",
            "monitor returned an unknown execution state",
        ));
    }
    let success = matches!(execution.state.as_str(), "running" | "done");
    let text = format!("Execution {}: {} ({label})", execution.id, execution.state);
    Ok(Output {
        data: json!({"authority":"monitor_execution","execution":execution}),
        text,
        exit: i32::from(!success),
    })
}
