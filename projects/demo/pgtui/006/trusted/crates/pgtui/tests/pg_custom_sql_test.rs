//! Trusted TASK-006 gate: custom SQL semantics over the simple-query protocol (D-023, D-025).

mod support;

use pgtui::db::{Cell, DbError, PREVIEW_LIMIT, PgSession, QueryOutcome};

#[tokio::test]
async fn select_returns_rows() {
    let (_container, params) = support::pg_container().await;
    let session = PgSession::connect(&params).await.expect("connect");

    let outcome = session
        .query("SELECT name FROM public.customers WHERE note IS NULL ORDER BY id")
        .await
        .expect("query");
    let QueryOutcome::Rows(rows) = outcome else {
        panic!("expected rows");
    };
    assert_eq!(rows.columns, vec!["name".to_string()]);
    assert_eq!(rows.rows.len(), 2);
    assert_eq!(rows.rows[0][0], Cell::Text("Bob".to_string()));
    assert_eq!(rows.rows[1][0], Cell::Text("Dee".to_string()));
}

#[tokio::test]
async fn command_complete_reports_affected() {
    let (_container, params) = support::pg_container().await;
    let session = PgSession::connect(&params).await.expect("connect");

    let outcome = session
        .query("UPDATE public.customers SET note = note WHERE id < 3")
        .await
        .expect("query");
    assert_eq!(outcome, QueryOutcome::Affected(2));

    session
        .query("UPDATE public.customers SET note = note WHERE id > 900")
        .await
        .expect("query affecting nothing");
}

#[tokio::test]
async fn multi_statement_rejected() {
    let (_container, params) = support::pg_container().await;
    let session = PgSession::connect(&params).await.expect("connect");

    let error = session
        .query("SELECT 1; SELECT 2")
        .await
        .expect_err("two statements are refused");
    assert_eq!(error, DbError::MultiStatement);
    assert_eq!(error.to_string(), "one statement at a time");
}

#[tokio::test]
async fn rows_capped_at_limit() {
    let (_container, params) = support::pg_container().await;
    let session = PgSession::connect(&params).await.expect("connect");

    let outcome = session
        .query("SELECT g FROM generate_series(1, 600) g")
        .await
        .expect("query");
    let QueryOutcome::Rows(rows) = outcome else {
        panic!("expected rows");
    };
    assert_eq!(rows.rows.len(), PREVIEW_LIMIT);
    assert_eq!(rows.rows[0][0], Cell::Text("1".to_string()));
}

#[tokio::test]
async fn syntax_error_reports_query_error() {
    let (_container, params) = support::pg_container().await;
    let session = PgSession::connect(&params).await.expect("connect");

    let error = session
        .query("SELECT nope FROM public.customers")
        .await
        .expect_err("column does not exist");
    assert!(matches!(error, DbError::Query(_)), "{error:?}");
    assert!(!error.to_string().is_empty());
}
