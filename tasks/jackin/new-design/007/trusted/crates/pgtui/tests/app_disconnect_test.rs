//! Trusted TASK-007 gate: disconnect flow and `Ctrl+C` ordering (D-030, D-033).

mod support;

use pgtui::app::{App, Effect, Msg, QueryKind, Screen};
use pgtui::db::QueryOutcome;

fn connected_app() -> App {
    let mut app = App::new();
    app.update(Msg::Connections(vec![
        support::saved_connection("alpha"),
        support::saved_connection("local"),
    ]));
    app.update(support::key('j'));
    app.update(Msg::Connected(Ok(support::fake_data::tables())));
    app
}

/// Connected with a preview grid and a custom-SQL result.
fn full_app() -> App {
    let mut app = connected_app();
    let effects = app.update(support::enter());
    let Effect::Query { kind, .. } = &effects[0] else {
        panic!("expected a query effect: {effects:?}");
    };
    let QueryKind::Preview(table) = kind else {
        panic!("expected a preview: {kind:?}");
    };
    app.update(Msg::QueryDone {
        kind: kind.clone(),
        result: Ok(QueryOutcome::Rows(support::fake_data::preview(table))),
    });
    app.sql_input = "select 1".to_string();
    app
}

#[test]
fn d_from_browser_emits_disconnect_effect() {
    let mut app = connected_app();
    let effects = app.update(support::key('d'));
    assert_eq!(effects, vec![Effect::Disconnect]);
    assert_eq!(app.screen, Screen::Browser, "effect, not transition");
}

#[test]
fn disconnected_resets_session_and_grids() {
    let mut app = full_app();
    assert!(app.update(Msg::Disconnected).is_empty());

    assert_eq!(app.screen, Screen::ConnectionList);
    assert!(app.session.is_none(), "session cleared");
    assert!(app.grid.is_none(), "preview grid cleared");
    assert!(app.sql_grid.is_none(), "custom grid cleared");
    assert_eq!(app.status, pgtui::app::Status::Help);
}

#[test]
fn disconnected_keeps_list_and_cursor() {
    let mut app = full_app();
    app.update(Msg::Disconnected);

    assert_eq!(app.connections.len(), 2, "saved list untouched");
    assert_eq!(app.list_cursor, 1, "cursor untouched");
    assert_eq!(
        app.connections[1].name, "local",
        "the same row is still selected"
    );
}

#[test]
fn disconnected_clears_sql_input() {
    let mut app = full_app();
    assert_eq!(app.sql_input, "select 1");

    app.update(Msg::Disconnected);
    assert_eq!(app.sql_input, "", "input cleared with the session");
}

#[test]
fn ctrl_c_while_connected_emits_disconnect_then_quit() {
    let mut app = full_app();
    let effects = app.update(support::ctrl('c'));
    assert_eq!(effects, vec![Effect::Disconnect, Effect::Quit]);

    let mut app = connected_app();
    app.update(Msg::Disconnected);
    let effects = app.update(support::ctrl('c'));
    assert_eq!(effects, vec![Effect::Quit], "already disconnected");
}
