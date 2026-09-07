//! Trusted TASK-006 gate: CustomSql rendering (D-034, D-060).

mod support;

use pgtui::app::{App, Msg, QueryKind, Screen};
use pgtui::db::QueryOutcome;

fn sql_app() -> App {
    let mut app = App::new();
    app.update(Msg::Connections(vec![support::saved_connection("local")]));
    app.update(Msg::Connected(Ok(support::fake_data::tables())));
    app.update(support::key('x'));
    assert_eq!(app.screen, Screen::CustomSql);
    app
}

fn with_results() -> App {
    let mut app = sql_app();
    for c in "select id, status from public.orders".chars() {
        app.update(support::key(c));
    }
    app.update(support::enter());
    app.update(Msg::QueryDone {
        kind: QueryKind::Custom,
        result: Ok(QueryOutcome::Rows(support::fake_data::orders())),
    });
    app
}

#[test]
fn empty_sql_screen_shows_input_and_hint() {
    let app = sql_app();
    let rendered = support::render_app(&app);

    assert!(rendered.contains(" SQL "), "input block title: {rendered}");
    assert!(rendered.contains("> "), "input prompt: {rendered}");
    assert!(
        rendered.contains(" Results "),
        "results block title: {rendered}"
    );
    assert!(
        rendered.contains("Type a query and press Enter"),
        "empty-state hint: {rendered}"
    );
    assert!(
        rendered.contains("Enter run  Up/Down rows  Esc back  Ctrl+C quit"),
        "help line: {rendered}"
    );
}

#[test]
fn results_block_renders_rows_and_count() {
    let app = with_results();
    let rendered = support::render_app(&app);

    assert!(
        rendered.contains(" Results  7 rows "),
        "result title: {rendered}"
    );
    for value in ["id", "status", "paid", "open", "refunded"] {
        assert!(rendered.contains(value), "{value}: {rendered}");
    }
    assert!(
        rendered.contains("select id, status from public.orders"),
        "input echo: {rendered}"
    );
}

#[test]
fn custom_grid_has_no_sort_markers() {
    let mut app = with_results();
    app.update(support::key('s'));
    app.update(support::key('h'));

    assert_eq!(app.sql_input.len(), 38, "s and h typed into the input");
    let rendered = support::render_app(&app);
    assert!(!rendered.contains("["), "no bracketed header: {rendered}");
    assert!(!rendered.contains(" ^"), "no asc marker: {rendered}");
    assert!(!rendered.contains(" v"), "no desc marker: {rendered}");

    let svg = support::render_app_svg(&app);
    assert!(svg.contains("Results"), "{svg}");
    assert_eq!(support::render_app_png_dims(&app), (900, 540));
}
