//! Local HTTP monitoring and tracked, dependency-aware execution.

pub mod adapter;
mod atomic_json;
mod scheduler;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use tokio::sync::{Semaphore, watch};
use tokio::task::JoinSet;

use crate::monitor::{Catalog, Eligibility, RunLock, Scope};

pub use scheduler::Execution;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub tasks_root: PathBuf,
    pub runs_root: PathBuf,
    pub adapter: Option<adapter::AdapterConfig>,
    pub concurrency: usize,
    /// Browser origin, including its port. No wildcard or reflected origins.
    pub origin: String,
}

struct Inner {
    config: ServerConfig,
    reservations: BTreeSet<String>,
    executions: Vec<Execution>,
    unavailable: Option<String>,
    _locks: (RunLock, RunLock),
}

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Mutex<Inner>>,
    workers: Arc<tokio::sync::Mutex<JoinSet<()>>>,
    slots: Arc<Semaphore>,
    requests: Arc<Semaphore>,
    cancel: watch::Sender<bool>,
    origin: Arc<str>,
}

impl AppState {
    /// Open roots, take the process lock, and reconcile durable execution before serving.
    pub async fn open(mut config: ServerConfig) -> anyhow::Result<Self> {
        anyhow::ensure!(
            (1..=32).contains(&config.concurrency),
            "concurrency must be between 1 and 32"
        );
        anyhow::ensure!(
            config.origin.starts_with("http://localhost:")
                || config.origin.starts_with("http://127.0.0.1:")
                || config.origin.starts_with("http://[::1]:"),
            "origin must name a local HTTP frontend"
        );
        let setup = tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&config.runs_root)?;
            anyhow::ensure!(
                !std::fs::symlink_metadata(&config.runs_root)?
                    .file_type()
                    .is_symlink(),
                "run root cannot be a symlink"
            );
            config.runs_root = config.runs_root.canonicalize()?;
            let catalog = Catalog::load(&config.tasks_root)?;
            config.tasks_root = catalog.root.clone();
            anyhow::ensure!(
                !config.runs_root.starts_with(&config.tasks_root)
                    && !config.tasks_root.starts_with(&config.runs_root),
                "task and run roots must not overlap"
            );
            let task_lock = RunLock::acquire(&config.tasks_root)?;
            let run_lock = RunLock::acquire(&config.runs_root)?;
            let binding = config.runs_root.join("catalog-root");
            if binding.exists() {
                anyhow::ensure!(
                    !std::fs::symlink_metadata(&binding)?
                        .file_type()
                        .is_symlink(),
                    "catalog binding cannot be a symlink"
                );
                anyhow::ensure!(
                    std::fs::read_to_string(&binding)? == config.tasks_root.to_string_lossy(),
                    "run root belongs to a different task catalog"
                );
            } else {
                anyhow::ensure!(
                    std::fs::read_dir(&config.runs_root)?
                        .all(|entry| entry.is_ok_and(|entry| entry.file_name() == ".monitor.lock")),
                    "unbound run root must be empty"
                );
                atomic_json::write_bytes(&binding, config.tasks_root.to_string_lossy().as_bytes())?;
            }
            let executions = scheduler::read_executions(&config.runs_root)?;
            anyhow::ensure!(
                executions.iter().all(|execution| execution
                    .task_ids
                    .iter()
                    .all(|id| catalog.tasks.contains_key(id))),
                "execution journal references a task outside this catalog"
            );
            Ok::<_, anyhow::Error>((config, (task_lock, run_lock), executions))
        })
        .await??;
        let (config, locks, executions) = setup;
        let (cancel, _) = watch::channel(false);
        let state = Self {
            slots: Arc::new(Semaphore::new(config.concurrency)),
            requests: Arc::new(Semaphore::new(64)),
            origin: Arc::from(config.origin.as_str()),
            inner: Arc::new(Mutex::new(Inner {
                config,
                reservations: BTreeSet::new(),
                executions,
                unavailable: None,
                _locks: locks,
            })),
            workers: Arc::new(tokio::sync::Mutex::new(JoinSet::new())),
            cancel,
        };
        scheduler::recover(&state).await?;
        state
            .refresh_availability()
            .await
            .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        Ok(state)
    }

    async fn access<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut Inner) -> anyhow::Result<T> + Send + 'static,
    ) -> Result<T, ApiError> {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            let mut inner = inner
                .lock()
                .map_err(|_| anyhow::anyhow!("monitor state poisoned"))?;
            operation(&mut inner)
        })
        .await
        .map_err(|error| ApiError::internal(error.into()))?
        .map_err(ApiError::catalog_invalid)
    }

    pub async fn shutdown(&self) {
        self.cancel.send_replace(true);
        let mut workers = self.workers.lock().await;
        while let Some(result) = workers.join_next().await {
            if let Err(error) = result {
                eprintln!("monitor worker failed during shutdown: {error}");
            }
        }
    }

    async fn snapshot(&self) -> Result<Value, ApiError> {
        self.refresh_availability().await?;
        self.access(|inner| {
            let mut catalog = Catalog::load(&inner.config.tasks_root)?;
            scheduler::enrich(&mut catalog, inner)?;
            let mut value = serde_json::to_value(&catalog)?;
            for task in catalog.tasks.values() {
                value["tasks"][&task.id]["eligibility"] = serde_json::to_value(eligibility(
                    inner,
                    &catalog,
                    &Scope::Task(task.id.clone()),
                ))?;
                value["tasks"][&task.id]["readiness"] =
                    serde_json::to_value(catalog.eligibility(&task.id))?;
                value["tasks"][&task.id]["latest_run"] = scheduler::latest_run(inner, &task.id);
            }
            for group in catalog.groups.values() {
                value["groups"][&group.id]["summary"] =
                    serde_json::to_value(catalog.aggregate(&group.tasks))?;
                value["groups"][&group.id]["eligibility"] = serde_json::to_value(eligibility(
                    inner,
                    &catalog,
                    &Scope::Group {
                        project: group.project_code.clone(),
                        group: group.code.clone(),
                    },
                ))?;
            }
            for project in catalog.projects.values() {
                let scope = Scope::Project(project.code.clone());
                value["projects"][&project.code]["summary"] =
                    serde_json::to_value(catalog.aggregate(&catalog.scope_tasks(&scope)?))?;
                value["projects"][&project.code]["eligibility"] =
                    serde_json::to_value(eligibility(inner, &catalog, &scope))?;
                value["projects"][&project.code]["group_count"] = json!(project.groups.len());
            }
            value["summary"] = serde_json::to_value(
                catalog.aggregate(&catalog.tasks.keys().cloned().collect::<Vec<_>>()),
            )?;
            value["topological_order"] = json!(catalog.topological_order()?);
            value["execution"] =
                json!({"available":inner.unavailable.is_none(),"reason":inner.unavailable});
            Ok(value)
        })
        .await
    }

    async fn refresh_availability(&self) -> Result<(), ApiError> {
        if *self.cancel.borrow() {
            return self.access(|inner|{inner.unavailable=Some("Execution halted or server shutting down. Restart the backend after resolving its logged failure.".into());Ok(())}).await;
        }
        let configured = self
            .access(|inner| {
                Ok(inner
                    .config
                    .adapter
                    .as_ref()
                    .map(|adapter| adapter.validate().is_ok()))
            })
            .await?;
        let reason=match configured {
            None=>Some("Execution unavailable: configure taskfmt, experiment config, and repository on the server."),
            Some(false)=>Some("Execution unavailable: taskfmt or experiment configuration is invalid."),
            Some(true)=>{
                let docker=tokio::time::timeout(std::time::Duration::from_secs(3),tokio::process::Command::new("docker").args(["info","--format","{{.ServerVersion}}"])
                    .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).kill_on_drop(true).status()).await;
                if matches!(docker,Ok(Ok(status)) if status.success()) {None}else{Some("Execution unavailable: Docker is not reachable.")}
            }
        }.map(str::to_owned);
        self.access(move |inner| {
            inner.unavailable = reason;
            Ok(())
        })
        .await
    }
}

fn eligibility(inner: &Inner, catalog: &Catalog, scope: &Scope) -> Eligibility {
    let denied = |reason: String| Eligibility {
        allowed: false,
        reason: Some(reason),
    };
    if let Some(reason) = &inner.unavailable {
        return denied(reason.clone());
    }
    match catalog.validate_scope(scope) {
        Err(error) => denied(error.to_string()),
        Ok(ids) if ids.iter().any(|id| inner.reservations.contains(id)) => {
            denied("An execution already owns this scope.".into())
        }
        Ok(_) => Eligibility {
            allowed: true,
            reason: None,
        },
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/catalog", get(catalog))
        .route("/api/executions", get(executions))
        .route("/api/run/task/{project}/{group}/{number}", post(run_task))
        .route("/api/run/group/{project}/{group}", post(run_group))
        .route("/api/run/project/{project}", post(run_project))
        .fallback(|| async {
            ApiError::new(StatusCode::NOT_FOUND, "not_found", "API route not found")
        })
        .method_not_allowed_fallback(|| async {
            ApiError::new(
                StatusCode::METHOD_NOT_ALLOWED,
                "method_not_allowed",
                "HTTP method not allowed",
            )
        })
        .layer(middleware::from_fn_with_state(
            state.clone(),
            browser_policy,
        ))
        .with_state(state)
}

async fn browser_policy(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let Ok(_request) = state.requests.clone().try_acquire_owned() else {
        return ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "busy",
            "Too many monitor requests; retry shortly",
        )
        .into_response();
    };
    let headers = request.headers();
    if headers
        .get("origin")
        .is_some_and(|origin| origin.as_bytes() != state.origin.as_bytes())
    {
        return ApiError::new(
            StatusCode::FORBIDDEN,
            "origin_denied",
            "Browser origin is not allowed",
        )
        .into_response();
    }
    // Requiring a non-simple header prevents cross-site form submissions even when Origin
    // is absent; DNS rebinding is additionally stopped by the explicit Host allowlist.
    if request.method() == Method::POST && headers.get("x-task-monitor").is_none_or(|v| v != "1") {
        return ApiError::new(
            StatusCode::FORBIDDEN,
            "request_denied",
            "Execution requires the monitor request header",
        )
        .into_response();
    }
    if let Some(host) = headers.get("host").and_then(|v| v.to_str().ok())
        && !(host.starts_with("127.0.0.1:")
            || host.starts_with("localhost:")
            || host.starts_with("[::1]:"))
    {
        return ApiError::new(StatusCode::FORBIDDEN, "host_denied", "Host is not allowed")
            .into_response();
    }
    if request.method() == Method::OPTIONS {
        let mut response = StatusCode::NO_CONTENT.into_response();
        cors(response.headers_mut(), &state.origin);
        return response;
    }
    let mut response = next.run(request).await;
    cors(response.headers_mut(), &state.origin);
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().expect("static header"));
    response
}

fn cors(headers: &mut HeaderMap, origin: &str) {
    if let Ok(origin) = origin.parse() {
        headers.insert("access-control-allow-origin", origin);
    }
    headers.insert(
        "access-control-allow-methods",
        "GET, POST, OPTIONS".parse().expect("static header"),
    );
    headers.insert(
        "access-control-allow-headers",
        "x-task-monitor, content-type"
            .parse()
            .expect("static header"),
    );
    headers.insert("vary", "Origin".parse().expect("static header"));
}

async fn catalog(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(state.snapshot().await?))
}
async fn executions(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    state
        .access(|inner| Ok(json!(inner.executions)))
        .await
        .map(Json)
}

async fn run_task(
    State(state): State<AppState>,
    path: Result<Path<(String, String, String)>, axum::extract::rejection::PathRejection>,
) -> Result<(StatusCode, Json<Execution>), ApiError> {
    let Path((project, group, number)) = path.map_err(|_| ApiError::invalid())?;
    let id = format!("{project}/{group}/{number}");
    crate::monitor::validate_id(&id).map_err(|_| ApiError::invalid())?;
    scheduler::start(state, Scope::Task(id))
        .await
        .map(|execution| (StatusCode::ACCEPTED, Json(execution)))
}
async fn run_group(
    State(state): State<AppState>,
    path: Result<Path<(String, String)>, axum::extract::rejection::PathRejection>,
) -> Result<(StatusCode, Json<Execution>), ApiError> {
    let Path((project, group)) = path.map_err(|_| ApiError::invalid())?;
    crate::monitor::validate_code(&project)
        .and_then(|()| crate::monitor::validate_code(&group))
        .map_err(|_| ApiError::invalid())?;
    scheduler::start(state, Scope::Group { project, group })
        .await
        .map(|execution| (StatusCode::ACCEPTED, Json(execution)))
}
async fn run_project(
    State(state): State<AppState>,
    path: Result<Path<String>, axum::extract::rejection::PathRejection>,
) -> Result<(StatusCode, Json<Execution>), ApiError> {
    let Path(project) = path.map_err(|_| ApiError::invalid())?;
    crate::monitor::validate_code(&project).map_err(|_| ApiError::invalid())?;
    scheduler::start(state, Scope::Project(project))
        .await
        .map(|execution| (StatusCode::ACCEPTED, Json(execution)))
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}
impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
    fn invalid() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_id",
            "Invalid canonical identifier",
        )
    }
    fn internal(error: anyhow::Error) -> Self {
        eprintln!("monitor operation failed: {error:#}");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Monitor operation failed. Inspect the server log.",
        )
    }
    fn catalog_invalid(error: anyhow::Error) -> Self {
        eprintln!("monitor catalog operation failed: {error:#}");
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "catalog_unavailable",
            "Catalog or execution data is invalid or unavailable. Inspect the server log.",
        )
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error":{"code":self.code,"message":self.message}})),
        )
            .into_response()
    }
}

#[cfg(test)]
mod integration_tests;
#[cfg(all(test, unix))]
mod supervisor_tests;
#[cfg(test)]
mod tests;
