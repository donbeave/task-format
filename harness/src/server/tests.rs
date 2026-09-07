use super::*;
use axum::body::{Body, to_bytes};
use axum::http::Request;
use tower::ServiceExt;

fn fixture(status: &str) -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let group = temp.path().join("tasks/jackin/design");
    let task = group.join("001");
    std::fs::create_dir_all(&task).unwrap();
    std::fs::write(
        temp.path().join("tasks/jackin/README.md"),
        "# Jackin\n\nProject",
    )
    .unwrap();
    std::fs::write(group.join("README.md"), "# Design\n\nGroup").unwrap();
    for file in ["README.md", "verify.toml"] {
        std::fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("testdata/example")
                .join(file),
            task.join(file),
        )
        .unwrap();
    }
    std::fs::write(
        task.join("task.toml"),
        format!("schema = \"task-meta/v1\"\nstatus = {status:?}\ndependencies = []\n"),
    )
    .unwrap();
    temp
}

fn config(temp: &tempfile::TempDir) -> ServerConfig {
    ServerConfig {
        tasks_root: temp.path().join("tasks"),
        runs_root: temp.path().join("runs"),
        adapter: None,
        concurrency: 2,
        origin: "http://127.0.0.1:5173".into(),
    }
}

#[tokio::test]
async fn snapshot_preserves_readiness_when_execution_unavailable() {
    let temp = fixture("pending");
    let state = AppState::open(config(&temp)).await.unwrap();
    let response = router(state)
        .oneshot(
            Request::builder()
                .uri("/api/catalog")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    let task = &body["tasks"]["jackin/design/001"];
    assert_eq!(
        (
            task["readiness"]["allowed"].as_bool(),
            task["eligibility"]["allowed"].as_bool()
        ),
        (Some(true), Some(false))
    );
    assert!(task.get("path").is_none());
    assert_eq!(body["summary"]["pending"], 1);
}

#[tokio::test]
async fn browser_write_policy_rejects_origin_form_and_rebinding() {
    let temp = fixture("pending");
    let app = router(AppState::open(config(&temp)).await.unwrap());
    for (header, value) in [
        ("origin", "https://attacker.invalid"),
        ("host", "attacker.invalid:3001"),
        ("x-task-monitor", "0"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/run/task/jackin/design/001")
                    .header(header, value)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert!(body["error"]["code"].is_string());
    }
}

#[tokio::test]
async fn disabled_execution_and_invalid_ids_have_typed_errors() {
    let temp = fixture("pending");
    let app = router(AppState::open(config(&temp)).await.unwrap());
    for (path, status) in [
        ("/api/run/task/jackin/design/001", StatusCode::CONFLICT),
        (
            "/api/run/task/jackin/design/not-a-number",
            StatusCode::BAD_REQUEST,
        ),
        ("/api/run/task/jackin/missing/001", StatusCode::NOT_FOUND),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("x-task-monitor", "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert!(body["error"]["message"].is_string());
    }
}

#[tokio::test]
async fn process_lock_and_catalog_binding_prevent_cross_catalog_recovery() {
    let first = fixture("pending");
    let second = fixture("pending");
    let state = AppState::open(config(&first)).await.unwrap();
    assert!(AppState::open(config(&first)).await.is_err());
    let mut conflicting = config(&second);
    conflicting.runs_root = first.path().join("runs");
    assert!(AppState::open(conflicting.clone()).await.is_err());
    drop(state);
    assert!(
        AppState::open(conflicting).await.is_err(),
        "durable root binding must survive lock release"
    );
}

#[tokio::test]
async fn stale_metadata_without_run_recovers_but_manual_done_is_rejected() {
    let stale = fixture("in_progress");
    let state = AppState::open(config(&stale)).await.unwrap();
    let body = state.snapshot().await.unwrap();
    assert_eq!(
        body["tasks"]["jackin/design/001"]["metadata"]["status"],
        "pending"
    );
    let forged = fixture("done");
    assert!(AppState::open(config(&forged)).await.is_err());
}

#[tokio::test]
async fn malformed_catalog_is_visible_after_startup() {
    let temp = fixture("pending");
    let state = AppState::open(config(&temp)).await.unwrap();
    std::fs::write(
        temp.path().join("tasks/jackin/design/001/task.toml"),
        "bad metadata",
    )
    .unwrap();
    let response = router(state)
        .oneshot(
            Request::builder()
                .uri("/api/catalog")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body =
        String::from_utf8(to_bytes(response.into_body(), 4096).await.unwrap().to_vec()).unwrap();
    assert!(!body.contains(temp.path().to_str().unwrap()));
}

#[tokio::test]
async fn overlapping_roots_are_rejected() {
    let temp = fixture("pending");
    let mut config = config(&temp);
    config.runs_root = config.tasks_root.join("runs");
    assert!(AppState::open(config).await.is_err());
}

#[tokio::test]
async fn partial_live_progress_is_visible_without_invalidating_execution() {
    let temp = fixture("pending");
    let state = AppState::open(config(&temp)).await.unwrap();
    state.access(|inner| {
        let artifact_id=uuid::Uuid::new_v4().to_string();
        let run_dir=inner.config.runs_root.join(&artifact_id).join("run");
        std::fs::create_dir_all(run_dir.join("progress"))?;
        let manifest:crate::runstate::Manifest=serde_json::from_value(json!({
            "run":"run","run_dir":run_dir,"container":"fixture","agent":"fixture","agent_kind":"codex","model":"fixture","effort":"low","task":"TASK-042","repo_url":"file:///fixture","base_sha":"base","session_id":"fixture","pane":"fixture","start":"fixture"
        }))?;
        manifest.save(&run_dir)?;
        std::fs::write(run_dir.join("progress/progress.md"),"---\nschema: progress/v1\n")?;
        let mut catalog=Catalog::load(&inner.config.tasks_root)?;
        catalog.start("jackin/design/001")?;
        inner.executions.push(Execution{id:uuid::Uuid::new_v4().to_string(),task_ids:vec!["jackin/design/001".into()],state:"running".into(),started:"fixture".into(),finished:None,tasks:std::collections::BTreeMap::from([("jackin/design/001".into(),scheduler::TaskRun{artifact_id,state:"running".into(),failure:None})])});
        Ok(())
    }).await.unwrap();
    let body = state.snapshot().await.unwrap();
    assert_eq!(
        body["tasks"]["jackin/design/001"]["metadata"]["status"],
        "in_progress"
    );
    assert!(
        body["tasks"]["jackin/design/001"]["progress"]["current_failure"]
            .as_str()
            .unwrap()
            .contains("temporarily unavailable")
    );
    assert!(!*state.cancel.borrow());
}

#[tokio::test]
async fn halted_execution_explains_restart_in_snapshot() {
    let temp = fixture("pending");
    let state = AppState::open(config(&temp)).await.unwrap();
    state.cancel.send_replace(true);
    let body = state.snapshot().await.unwrap();
    assert_eq!(body["execution"]["available"], false);
    assert!(
        body["tasks"]["jackin/design/001"]["eligibility"]["reason"]
            .as_str()
            .unwrap()
            .contains("Restart")
    );
}

#[tokio::test]
async fn encoded_traversal_is_rejected_before_execution_admission() {
    let temp = fixture("pending");
    let state = AppState::open(config(&temp)).await.unwrap();
    let app = router(state.clone());
    for path in [
        "/api/run/task/jackin/design/%2E%2E",
        "/api/run/task/jackin/design/%2e%2e%2f001",
        "/api/run/task/jackin/design/%252e%252e%252f001",
        "/api/run/task/jackin%2f..%2foutside/design/001",
        "/api/run/task/jackin/design%5c..%5coutside/001",
        "/api/run/task/jackin/design/%00",
        "/api/run/task/jackin/design/%FF",
        "/api/run/task/jackin/design/../../outside/001",
        "/api/run/group/jackin/%2e%2e",
        "/api/run/project/%2e%2e",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("x-task-monitor", "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                response.status(),
                StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND
            ),
            "{path}: unexpected response {}",
            response.status()
        );
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert!(
            body["error"]["code"].is_string(),
            "{path}: missing typed error"
        );
    }
    state
        .access(|inner| {
            assert!(inner.executions.is_empty());
            assert!(inner.reservations.is_empty());
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(
        Catalog::load(&temp.path().join("tasks")).unwrap().tasks["jackin/design/001"]
            .metadata
            .status,
        crate::monitor::Status::Pending
    );
}
