//! Trusted TASK-007 gate: the backend really goes away on disconnect (D-024, D-012).

mod support;

use pgtui::db::{Cell, PgSession, QueryOutcome};
use std::time::{Duration, Instant};

async fn pgtui_backends(admin: &PgSession) -> u64 {
    // The observing session is itself a `PgSession` and so announces `application_name = 'pgtui'`
    // (D-024); exclude the backend that is running this count, by identity and not by arithmetic.
    let outcome = admin
        .query(
            "SELECT count(*) FROM pg_stat_activity \
             WHERE application_name = 'pgtui' AND pid <> pg_backend_pid()",
        )
        .await
        .expect("count query");
    let QueryOutcome::Rows(rows) = outcome else {
        panic!("expected rows");
    };
    match &rows.rows[0][0] {
        Cell::Text(text) => text.parse().expect("count is numeric"),
        Cell::Null => 0,
    }
}

#[tokio::test]
async fn disconnect_closes_backend() {
    let (_container, params) = support::pg_container().await;
    let session = PgSession::connect(&params).await.expect("app connect");
    let admin = PgSession::connect(&params).await.expect("admin connect");

    assert_eq!(
        pgtui_backends(&admin).await,
        1,
        "the app connection announces itself as 'pgtui'"
    );

    session.disconnect().await.expect("disconnect");

    let deadline = Instant::now() + Duration::from_secs(5);
    while pgtui_backends(&admin).await > 0 {
        assert!(
            Instant::now() < deadline,
            "backend still registered after disconnect"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
