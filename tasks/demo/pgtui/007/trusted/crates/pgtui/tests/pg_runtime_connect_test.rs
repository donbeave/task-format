//! Trusted TASK-004 gate: `Effect::Connect` end to end (D-011, D-012).

mod support;

use pgtui::app::{Effect, Msg};
use pgtui::runtime::{self, Runtime};

#[tokio::test]
async fn connect_effect_replies_seed_tables() {
    let (container, params) = support::pg_container().await;
    let (_dir, store) = support::temp_store().await;
    let mut runtime = Runtime::new(store);
    assert!(runtime.session.is_none());

    let saved = support::saved_connection_at("local", &params);
    let reply = runtime::execute(&mut runtime, Effect::Connect(saved))
        .await
        .expect("runtime replies");

    match reply {
        Msg::Connected(Ok(tables)) => assert_eq!(tables, support::fake_data::tables()),
        other => panic!("unexpected reply: {other:?}"),
    }
    assert!(
        runtime.session.is_some(),
        "the runtime keeps the session open"
    );

    drop(container);
}
