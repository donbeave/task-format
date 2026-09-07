//! Trusted TASK-002 gate: connection-list state machine (D-010, D-011, D-031).

mod support;

use pgtui::app::{App, Effect, Msg, Screen};
use pgtui::store::SavedConnection;

fn list_app(names: &[&str]) -> App {
    let connections: Vec<SavedConnection> = names
        .iter()
        .copied()
        .map(support::saved_connection)
        .collect();
    let mut app = App::new();
    let effects = app.update(Msg::Connections(connections));
    assert!(effects.is_empty(), "loading the list emits nothing");
    app
}

#[test]
fn connections_msg_populates_list() {
    let app = list_app(&["alpha", "beta"]);
    assert_eq!(app.screen, Screen::ConnectionList);
    assert_eq!(app.connections.len(), 2);
    assert_eq!(app.connections[1].name, "beta");
    assert_eq!(app.list_cursor, 0);
    assert_eq!(app.status, pgtui::app::Status::Help);
    assert!(!app.quit_requested);
}

#[test]
fn j_clamps_at_end() {
    let mut app = list_app(&["alpha", "beta"]);
    for _ in 0..4 {
        app.update(support::key('j'));
    }
    assert_eq!(app.list_cursor, 1, "cursor stops at the last row");

    app.update(support::down());
    assert_eq!(app.list_cursor, 1, "Down stops at the last row too");
}

#[test]
fn k_clamps_at_start() {
    let mut app = list_app(&["alpha", "beta"]);
    app.update(support::key('j'));
    assert_eq!(app.list_cursor, 1);

    app.update(support::key('k'));
    app.update(support::key('k'));
    app.update(support::key('k'));
    assert_eq!(app.list_cursor, 0, "cursor stops at the first row");

    app.update(support::up());
    assert_eq!(app.list_cursor, 0, "Up stops at the first row too");
}

#[test]
fn q_emits_quit() {
    let mut app = list_app(&["alpha"]);
    let effects = app.update(support::key('q'));
    assert_eq!(effects, vec![Effect::Quit]);
    assert!(app.quit_requested);
}

#[test]
fn ctrl_c_emits_quit() {
    let mut app = list_app(&["alpha"]);
    let effects = app.update(support::ctrl('c'));
    assert_eq!(effects, vec![Effect::Quit], "not connected: quit only");
    assert!(app.quit_requested);
}
