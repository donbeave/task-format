//! Trusted TASK-005 gate: grid rendering in the Browser main pane (D-060).

mod support;

use pgtui::app::{App, Effect, Msg, QueryKind, Screen};
use pgtui::db::QueryOutcome;

fn browser_app(table_index: usize) -> App {
    let mut app = App::new();
    app.update(Msg::Connections(vec![support::saved_connection("local")]));
    app.update(Msg::Connected(Ok(support::fake_data::tables())));
    for _ in 0..table_index {
        app.update(support::key('j'));
    }
    app
}

fn load_preview(app: &mut App) {
    let effects = app.update(support::enter());
    assert_eq!(effects.len(), 1, "{effects:?}");
    let Effect::Query { kind, .. } = &effects[0] else {
        panic!("expected a query effect: {effects:?}");
    };
    let QueryKind::Preview(table) = kind else {
        panic!("expected a preview: {kind:?}");
    };
    let rows = support::fake_data::preview(table);
    app.update(Msg::QueryDone {
        kind: kind.clone(),
        result: Ok(QueryOutcome::Rows(rows)),
    });
}

fn customers_app() -> App {
    let mut app = browser_app(1);
    load_preview(&mut app);
    app
}

#[test]
fn unsorted_grid_renders_headers_and_rows() {
    let app = customers_app();
    let rendered = support::render_app(&app);

    assert!(
        rendered.contains(" public.customers  5 rows  limit 500 "),
        "main pane title: {rendered}"
    );
    assert!(
        rendered.contains(" Tables (4) "),
        "sidebar title: {rendered}"
    );
    for column in ["id", "name", "balance", "signup_date", "note"] {
        assert!(rendered.contains(column), "{column}: {rendered}");
    }
    assert!(rendered.contains("Ada"), "{rendered}");
    assert!(rendered.contains("10.50"), "{rendered}");
    assert!(
        rendered.contains("NULL"),
        "Cell::Null renders as NULL: {rendered}"
    );
    assert_eq!(app.screen, Screen::Browser);
}

#[test]
fn cursor_column_header_is_bracketed() {
    let mut app = customers_app();
    app.update(support::key('l'));
    app.update(support::key('l'));

    let rendered = support::render_app(&app);
    assert!(rendered.contains("[balance]"), "{rendered}");
    assert!(
        !rendered.contains("[id]"),
        "only the cursor column: {rendered}"
    );
}

#[test]
fn sorted_column_header_gets_arrow() {
    let mut app = customers_app();
    app.update(support::key('l'));
    app.update(support::key('l'));

    app.update(support::key('s'));
    assert!(
        support::render_app(&app).contains("[balance ^]"),
        "ascending: {}",
        support::render_app(&app)
    );

    app.update(support::key('s'));
    assert!(
        support::render_app(&app).contains("[balance v]"),
        "descending: {}",
        support::render_app(&app)
    );

    app.update(support::key('s'));
    assert!(
        support::render_app(&app).contains("[balance]"),
        "unsorted again: {}",
        support::render_app(&app)
    );
}

#[test]
fn grid_rasterizes_with_status_help() {
    let app = customers_app();
    let rendered = support::render_app(&app);
    assert!(
        rendered
            .contains("Tab focus  j/k move  Enter preview  h/l col  s sort  x sql  d disconnect"),
        "help line: {rendered}"
    );

    let svg = support::render_app_svg(&app);
    assert!(svg.contains("balance"), "{svg}");
    assert!(svg.contains("Ada"), "{svg}");
    assert_eq!(support::render_app_png_dims(&app), (900, 540));
}

#[test]
fn sidebar_table_name_is_untouched_by_preview() {
    let mut app = customers_app();
    app.update(support::key('j'));
    app.update(support::key('j'));

    let rendered = support::render_app(&app);
    let row = rendered
        .split('\n')
        .find(|row| row.contains("public.orders"))
        .expect("orders row");
    assert!(row.contains("> "), "cursor row: {row:?}");
    assert!(
        rendered.contains("audit.events"),
        "sidebar intact: {rendered}"
    );
}
