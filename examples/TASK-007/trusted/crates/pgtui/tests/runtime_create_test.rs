//! Trusted TASK-003 gate: save effect against the real store (D-011, D-022, D-032).

mod support;

use pgtui::app::{App, Effect, Field, Msg, Screen, Status};
use pgtui::runtime::{self, Runtime};
use pgtui::store::StoreError;

fn focus_field(app: &mut App, field: Field) {
    for _ in 0..8 {
        if app.form.focus == field {
            return;
        }
        app.update(support::tab());
    }
    panic!("focus never reached {field:?}");
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.update(support::key(c));
    }
}

/// A form filled with the connection `name`, ready to be saved with `Enter`.
fn filled_form_app(name: &str) -> App {
    let mut app = App::new();
    app.update(Msg::Connections(Vec::new()));
    app.update(support::key('n'));
    focus_field(&mut app, Field::Name);
    type_text(&mut app, name);
    focus_field(&mut app, Field::Host);
    type_text(&mut app, "db.example.com");
    focus_field(&mut app, Field::Port);
    type_text(&mut app, "5432");
    focus_field(&mut app, Field::Database);
    type_text(&mut app, "appdb");
    focus_field(&mut app, Field::User);
    type_text(&mut app, "alice");
    app
}

#[tokio::test]
async fn save_connection_effect_persists_row() {
    let (_dir, store) = support::temp_store().await;
    let mut runtime = Runtime::new(store);

    let effects = filled_form_app("prod").update(support::enter());
    let Effect::SaveConnection(new) = &effects[0] else {
        panic!("expected SaveConnection, got {effects:?}");
    };
    let expected = new.clone();

    let reply = runtime::execute(&mut runtime, effects[0].clone()).await;
    match reply.expect("runtime replies") {
        Msg::Saved(Ok(saved)) => {
            assert_eq!(saved.name, "prod");
            assert_eq!(saved.host, "db.example.com");
            assert_eq!(saved.port, 5432);
            assert_eq!(saved.dbname, "appdb");
            assert_eq!(saved.username, "alice");
            assert_eq!(saved.password, "");
        }
        other => panic!("unexpected reply: {other:?}"),
    }

    let list = runtime.store.list().await.expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, expected.name);
}

#[tokio::test]
async fn saved_ok_reloads_list_and_selects_row() {
    let (_dir, store) = support::temp_store().await;
    let mut runtime = Runtime::new(store);
    let mut app = filled_form_app("prod");

    let effects = app.update(support::enter());
    assert_eq!(effects.len(), 1);

    let reply = runtime::execute(&mut runtime, effects[0].clone())
        .await
        .expect("runtime replies");
    let effects = app.update(reply);
    assert_eq!(effects, vec![Effect::LoadConnections], "Saved(Ok) reloads");

    let list = runtime.store.list().await.expect("list");
    assert_eq!(list.len(), 1);
    assert!(app.update(Msg::Connections(list)).is_empty());

    assert_eq!(app.screen, Screen::ConnectionList);
    assert_eq!(app.list_cursor, 0, "cursor on the new row");
    assert_eq!(app.connections[0].name, "prod");
}

#[tokio::test]
async fn duplicate_name_save_reports_error() {
    let (_dir, store) = support::temp_store().await;
    let mut runtime = Runtime::new(store);
    runtime
        .store
        .insert(support::new_connection("prod"))
        .await
        .expect("seed row");

    let mut app = filled_form_app("prod");
    let effects = app.update(support::enter());
    assert_eq!(effects.len(), 1);

    let reply = runtime::execute(&mut runtime, effects[0].clone())
        .await
        .expect("runtime replies");
    match &reply {
        Msg::Saved(Err(StoreError::DuplicateName(_))) => {}
        other => panic!("unexpected reply: {other:?}"),
    }

    assert!(app.update(reply).is_empty());
    assert_eq!(app.screen, Screen::CreateConnection, "form is kept");
    assert_eq!(app.form.name, "prod", "typed value is kept");
    assert_eq!(app.status, Status::Error("name already exists".to_string()));
    assert_eq!(runtime.store.list().await.expect("list").len(), 1);
}
