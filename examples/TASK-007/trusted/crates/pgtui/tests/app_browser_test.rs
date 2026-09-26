//! Trusted TASK-004 gate: Browser screen state after connecting (D-011, D-033).

mod support;

use pgtui::app::{App, Effect, Focus, Msg, Screen};

fn list_app() -> App {
    let mut app = App::new();
    app.update(Msg::Connections(vec![support::saved_connection("local")]));
    app
}

/// Connection `local` connected to the four seed tables.
fn connected_app() -> App {
    let mut app = list_app();
    let effects = app.update(support::enter());
    assert_eq!(
        effects,
        vec![Effect::Connect(support::saved_connection("local"))]
    );
    let tables = support::fake_data::tables();
    assert!(app.update(Msg::Connected(Ok(tables))).is_empty());
    app
}

#[test]
fn enter_on_list_emits_connect_effect() {
    let mut app = list_app();
    let effects = app.update(support::enter());
    assert_eq!(
        effects,
        vec![Effect::Connect(support::saved_connection("local"))],
        "the selected row is connected"
    );
    assert_eq!(app.screen, Screen::ConnectionList, "effect, not transition");
}

#[test]
fn connected_ok_enters_browser() {
    let app = connected_app();
    assert_eq!(app.screen, Screen::Browser);

    let session = app.session.as_ref().expect("session view");
    assert_eq!(session.name, "local");
    assert_eq!(session.tables, support::fake_data::tables());
    assert_eq!(session.sidebar_cursor, 0);
    assert_eq!(session.focus, Focus::Sidebar);
    assert!(app.grid.is_none());
}

#[test]
fn connected_err_stays_on_list() {
    let mut app = list_app();
    app.update(support::enter());

    let effects = app.update(Msg::Connected(Err(pgtui::db::DbError::Connect(
        "connection refused".to_string(),
    ))));
    assert!(effects.is_empty());

    assert_eq!(app.screen, Screen::ConnectionList);
    assert!(app.session.is_none());
    assert_eq!(app.connections.len(), 1, "list untouched");
    assert_eq!(
        app.status,
        pgtui::app::Status::Error("connection refused".to_string())
    );
}

#[test]
fn sidebar_j_k_clamp() {
    let mut app = connected_app();
    for _ in 0..9 {
        app.update(support::key('j'));
    }
    let cursor = app.session.as_ref().expect("session").sidebar_cursor;
    assert_eq!(cursor, 3, "stops at the last table");

    for _ in 0..9 {
        app.update(support::key('k'));
    }
    let cursor = app.session.as_ref().expect("session").sidebar_cursor;
    assert_eq!(cursor, 0, "stops at the first table");

    let mut app = connected_app();
    app.update(support::down());
    app.update(support::down());
    assert_eq!(app.session.as_ref().expect("session").sidebar_cursor, 2);
    app.update(support::up());
    assert_eq!(app.session.as_ref().expect("session").sidebar_cursor, 1);
}

#[test]
fn tab_without_grid_keeps_sidebar_focus() {
    let mut app = connected_app();
    app.update(support::tab());
    let session = app.session.as_ref().expect("session");
    assert_eq!(session.focus, Focus::Sidebar, "no grid yet: Tab is a no-op");
    assert_eq!(app.screen, Screen::Browser);
}

#[test]
fn enter_on_empty_sidebar_emits_nothing() {
    let mut app = list_app();
    app.update(Msg::Connected(Ok(Vec::new())));
    assert_eq!(app.screen, Screen::Browser);
    assert!(app.session.as_ref().expect("session").tables.is_empty());

    let effects = app.update(support::enter());
    assert!(effects.is_empty(), "nothing selected: {effects:?}");
}
