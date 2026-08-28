//! Trusted TASK-004 gate: PostgreSQL session, connect + table list (D-023..D-026, D-072).

mod support;

use pgtui::db::{ConnParams, DbError, PgSession};
use std::time::{Duration, Instant};

#[tokio::test]
async fn lists_seed_tables() {
    let (_container, params) = support::pg_container().await;
    let session = PgSession::connect(&params).await.expect("connect");
    let tables = session.list_tables().await.expect("tables");
    assert_eq!(tables, support::fake_data::tables(), "D-025 server order");
}

#[tokio::test]
async fn refused_port_errors_fast() {
    let params = ConnParams {
        host: "127.0.0.1".to_string(),
        port: 1,
        dbname: "postgres".to_string(),
        user: "postgres".to_string(),
        password: "postgres".to_string(),
    };

    let started = Instant::now();
    let error = match PgSession::connect(&params).await {
        Ok(_) => panic!("nothing listens on port 1"),
        Err(error) => error,
    };
    let elapsed = started.elapsed();
    assert!(
        matches!(error, DbError::Connect(_) | DbError::Timeout),
        "unexpected error: {error:?}"
    );
    assert!(
        elapsed < Duration::from_secs(6),
        "5 s timeout + slack, took {elapsed:?}"
    );
}

#[tokio::test]
async fn bad_password_reports_connect_error() {
    let (_container, params) = support::pg_container().await;
    let bad = ConnParams {
        password: "definitely-not-the-password".to_string(),
        ..params
    };

    let error = match PgSession::connect(&bad).await {
        Ok(_) => panic!("wrong password must not connect"),
        Err(error) => error,
    };
    assert!(
        matches!(error, DbError::Connect(_)),
        "unexpected error: {error:?}"
    );
    assert!(!error.to_string().is_empty(), "error carries a message");
}

#[test]
fn quote_ident_doubles_embedded_quotes() {
    assert_eq!(pgtui::db::quote_ident("customers"), "\"customers\"");
    assert_eq!(
        pgtui::db::quote_ident("we\"ird.table"),
        "\"we\"\"ird.table\""
    );
}
