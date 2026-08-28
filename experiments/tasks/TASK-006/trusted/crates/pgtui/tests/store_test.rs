//! Trusted TASK-002 gate: the Turso connection store (D-020..D-022).

mod support;

use pgtui::store::{ConnectionStore, NewConnection, StoreError};

fn new_conn(name: &str) -> NewConnection {
    support::new_connection(name)
}

#[tokio::test]
async fn open_creates_schema() {
    let (_dir, store) = support::temp_store().await;
    assert_eq!(store.list().await.expect("list after open"), Vec::new());
}

#[tokio::test]
async fn insert_then_list_sorted_by_name() {
    let (_dir, store) = support::temp_store().await;
    for name in ["zeta", "alpha", "mid"] {
        let saved = store.insert(new_conn(name)).await.expect("insert");
        assert_eq!(saved.name, name);
        assert_eq!(saved.host, "example.com");
        assert_eq!(saved.port, 5432);
        assert_eq!(saved.dbname, "appdb");
        assert_eq!(saved.username, "alice");
        assert_eq!(saved.password, "s3cr3t");
    }

    let list = store.list().await.expect("list");
    let names: Vec<&str> = list.iter().map(|saved| saved.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "mid", "zeta"], "ORDER BY name ASC");

    for saved in &list {
        let stamp = saved.created_at.as_str();
        assert_eq!(stamp.len(), 20, "RFC 3339 UTC seconds: {stamp}");
        let bytes = stamp.as_bytes();
        assert!(
            bytes[4] == b'-' && bytes[7] == b'-' && bytes[10] == b'T' && bytes[19] == b'Z',
            "RFC 3339 UTC: {stamp}"
        );
    }
}

#[tokio::test]
async fn reopen_persists() {
    let (dir, store) = support::temp_store().await;
    let saved = store.insert(new_conn("prod")).await.expect("insert");
    drop(store);

    let reopened = ConnectionStore::open(&dir.path().join("connections.db"))
        .await
        .expect("reopen");
    assert_eq!(reopened.list().await.expect("list"), vec![saved]);
}

#[test]
fn display_dsn_hides_password() {
    let saved = support::saved_connection("local");
    assert_eq!(saved.display_dsn(), "alice@example.com:5432/appdb");
    assert!(!saved.display_dsn().contains("s3cr3t"));
}

#[tokio::test]
async fn duplicate_name_rejected() {
    let (_dir, store) = support::temp_store().await;
    store.insert(new_conn("dup")).await.expect("first insert");

    let error = store
        .insert(new_conn("dup"))
        .await
        .expect_err("duplicate name must fail");
    assert!(
        matches!(error, StoreError::DuplicateName(_)),
        "unexpected error: {error:?}"
    );
    assert_eq!(store.list().await.expect("list").len(), 1);
}
