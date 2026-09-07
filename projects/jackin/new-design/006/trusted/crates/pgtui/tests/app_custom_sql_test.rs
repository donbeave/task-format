//! Trusted TASK-006 gate: CustomSql screen state (D-011, D-025, D-034, D-050).

mod support;

use pgtui::app::{App, Effect, Msg, QueryKind, Screen, Status};
use pgtui::db::{Cell, QueryOutcome, ResultSet};

fn browser_app() -> App {
    let mut app = App::new();
    app.update(Msg::Connections(vec![support::saved_connection("local")]));
    app.update(Msg::Connected(Ok(support::fake_data::tables())));
    app
}

fn sql_app(input: &str) -> App {
    let mut app = browser_app();
    app.update(support::key('x'));
    assert_eq!(app.screen, Screen::CustomSql);
    for c in input.chars() {
        app.update(support::key(c));
    }
    app
}

fn fake_rows() -> ResultSet {
    support::fake_data::orders()
}

#[test]
fn x_from_browser_opens_sql_screen() {
    let app = browser_app();
    let before = app.session.as_ref().expect("session").name.clone();

    let mut app = app;
    app.update(support::key('x'));

    assert_eq!(app.screen, Screen::CustomSql);
    assert_eq!(app.sql_input, "");
    assert!(app.sql_grid.is_none());
    assert_eq!(app.session.as_ref().expect("session").name, before);
}

#[test]
fn printable_chars_and_backspace_edit_input() {
    let mut app = sql_app("select 1");
    assert_eq!(app.sql_input, "select 1");

    app.update(support::backspace());
    assert_eq!(app.sql_input, "select ");

    for _ in 0..20 {
        app.update(support::backspace());
    }
    assert_eq!(app.sql_input, "", "backspace on empty input is a no-op");
}

#[test]
fn enter_with_empty_input_emits_nothing() {
    let mut app = sql_app("   ");
    let effects = app.update(support::enter());
    assert!(effects.is_empty(), "blank input: {effects:?}");
    assert_eq!(app.screen, Screen::CustomSql);
}

#[test]
fn enter_emits_custom_query_effect() {
    let mut app = sql_app("select 1 as a;");
    let effects = app.update(support::enter());

    assert_eq!(
        effects,
        vec![Effect::Query {
            kind: QueryKind::Custom,
            sql: "select 1 as a".to_string(),
        }],
        "one trailing semicolon is stripped, whitespace trimmed"
    );
    assert_eq!(app.screen, Screen::CustomSql);
}

#[test]
fn rows_done_builds_plain_sql_grid() {
    let mut app = sql_app("select * from orders");
    app.update(support::enter());

    let effects = app.update(Msg::QueryDone {
        kind: QueryKind::Custom,
        result: Ok(QueryOutcome::Rows(fake_rows())),
    });
    assert!(effects.is_empty(), "{effects:?}");

    let grid = app.sql_grid.as_ref().expect("sql grid");
    assert_eq!(grid.columns, fake_rows().columns);
    assert_eq!(grid.visible_rows().len(), 7);
    assert_eq!(grid.sort, None, "the custom grid never sorts");
    assert_eq!(grid.col_cursor, 0, "no column cursor");
    assert_eq!(grid.row_cursor, 0);
    assert_eq!(app.screen, Screen::CustomSql);
}

#[test]
fn capped_rows_show_info_status() {
    let mut app = sql_app("select * from big");
    app.update(support::enter());

    let mut rows = fake_rows();
    rows.rows = (0..600)
        .map(|index| {
            vec![
                Cell::Text(index.to_string()),
                Cell::Text("1".to_string()),
                Cell::Text("5.00".to_string()),
                Cell::Text("paid".to_string()),
            ]
        })
        .collect();
    app.update(Msg::QueryDone {
        kind: QueryKind::Custom,
        result: Ok(QueryOutcome::Rows(rows)),
    });

    assert_eq!(
        app.status,
        Status::Info("showing first 500 rows".to_string())
    );
    assert_eq!(
        app.sql_grid
            .as_ref()
            .expect("sql grid")
            .visible_rows()
            .len(),
        500
    );
}

#[test]
fn affected_done_shows_ok_info() {
    let mut app = sql_app("update customers set note = 'x'");
    app.update(support::enter());

    let effects = app.update(Msg::QueryDone {
        kind: QueryKind::Custom,
        result: Ok(QueryOutcome::Affected(3)),
    });
    assert!(effects.is_empty());

    assert_eq!(app.status, Status::Info("ok: 3 rows affected".to_string()));
    assert!(app.sql_grid.is_none(), "no grid for a non-row result");
}

#[test]
fn multi_statement_sets_error() {
    let mut app = sql_app("select 1; select 2");
    app.update(support::enter());

    let effects = app.update(Msg::QueryDone {
        kind: QueryKind::Custom,
        result: Err(pgtui::db::DbError::MultiStatement),
    });
    assert!(effects.is_empty());

    assert_eq!(
        app.status,
        Status::Error("one statement at a time".to_string())
    );
    assert!(app.sql_grid.is_none());
}

#[test]
fn esc_returns_to_browser_and_retains_input() {
    let mut app = sql_app("select 1");
    app.update(Msg::QueryDone {
        kind: QueryKind::Custom,
        result: Ok(QueryOutcome::Rows(fake_rows())),
    });

    app.update(support::esc());
    assert_eq!(app.screen, Screen::Browser);
    assert_eq!(app.sql_input, "select 1", "input is retained");
    assert!(app.sql_grid.is_some(), "result grid is retained");

    app.update(support::key('x'));
    assert_eq!(app.screen, Screen::CustomSql);
    assert_eq!(app.sql_input, "select 1");
    assert!(app.sql_grid.is_some(), "result still on screen");
}
