//! Trusted TASK-005 gate: preview query over the simple-query protocol (D-023, D-025, D-072).

mod support;

use pgtui::db::{Cell, DbError, PREVIEW_LIMIT, PgSession, QueryOutcome, TableRef};

fn preview_sql(table: &TableRef) -> String {
    format!(
        "SELECT * FROM {}.{} LIMIT 500",
        pgtui::db::quote_ident(&table.schema),
        pgtui::db::quote_ident(&table.name)
    )
}

#[tokio::test]
async fn preview_matches_fake() {
    let (_container, params) = support::pg_container().await;
    let session = PgSession::connect(&params).await.expect("connect");

    for table in support::fake_data::tables() {
        let sql = preview_sql(&table);
        let outcome = session.query(&sql).await.expect("query");
        let QueryOutcome::Rows(rows) = outcome else {
            panic!("expected rows for {table}");
        };
        let fake = support::fake_data::preview(&table);
        assert_eq!(rows.columns, fake.columns, "columns of {table}");
        assert_eq!(
            support::fake_data::sorted_rows(&rows),
            support::fake_data::sorted_rows(&fake),
            "rows of {table} as a multiset"
        );
    }
}

#[tokio::test]
async fn seed_null_cells_are_null() {
    let (_container, params) = support::pg_container().await;
    let session = PgSession::connect(&params).await.expect("connect");

    let outcome = session
        .query("SELECT id, note FROM public.customers ORDER BY id")
        .await
        .expect("query");
    let QueryOutcome::Rows(rows) = outcome else {
        panic!("expected rows");
    };

    assert_eq!(rows.rows.len(), 5);
    assert_eq!(rows.rows[0][1], Cell::Text("vip".to_string()));
    assert_eq!(rows.rows[1][1], Cell::Null, "Bob has no note");
    assert_eq!(rows.rows[3][1], Cell::Null, "Dee has no note");
    assert_eq!(rows.rows[4][1], Cell::Text("tie".to_string()));
}

#[tokio::test]
async fn limit_applied_on_600_row_table() {
    let (_container, params) = support::pg_container().await;
    let session = PgSession::connect(&params).await.expect("connect");

    session
        .query("CREATE TABLE public.big AS SELECT g AS id FROM generate_series(1, 600) g")
        .await
        .expect("create table");

    let outcome = session
        .query("SELECT * FROM \"public\".\"big\" LIMIT 500")
        .await
        .expect("query");
    let QueryOutcome::Rows(rows) = outcome else {
        panic!("expected rows");
    };
    assert_eq!(rows.rows.len(), PREVIEW_LIMIT, "capped at PREVIEW_LIMIT");
    assert_eq!(rows.rows[0][0], Cell::Text("1".to_string()));

    session
        .query("DROP TABLE public.big")
        .await
        .expect("drop table");
}

#[tokio::test]
async fn unknown_table_reports_query_error() {
    let (_container, params) = support::pg_container().await;
    let session = PgSession::connect(&params).await.expect("connect");

    let error = session
        .query("SELECT * FROM \"public\".\"nope\" LIMIT 500")
        .await
        .expect_err("relation does not exist");
    assert!(matches!(error, DbError::Query(_)), "{error:?}");
}
