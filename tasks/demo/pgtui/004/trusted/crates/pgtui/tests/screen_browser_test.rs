//! Trusted TASK-004 gate: Browser sidebar rendering (D-060).

mod support;

use pgtui::app::{App, Effect, Msg};

fn connected_app() -> App {
    let mut app = App::new();
    app.update(Msg::Connections(vec![support::saved_connection("local")]));
    let effects = app.update(support::enter());
    assert_eq!(
        effects,
        vec![Effect::Connect(support::saved_connection("local"))]
    );
    app.update(Msg::Connected(Ok(support::fake_data::tables())));
    app
}

#[test]
fn sidebar_lists_tables_with_focus_marker() {
    let app = connected_app();
    let rendered = support::render_app(&app);

    assert!(
        rendered.contains(" Tables (4)* "),
        "focused sidebar: {rendered}"
    );
    assert!(rendered.contains(" local "), "main pane title: {rendered}");
    for table in [
        "audit.events",
        "public.customers",
        "public.empty_table",
        "public.orders",
    ] {
        assert!(rendered.contains(table), "{table}: {rendered}");
    }
    assert!(
        rendered
            .contains("Tab focus  j/k move  Enter preview  h/l col  s sort  x sql  d disconnect"),
        "help line: {rendered}"
    );

    let row = rendered
        .split('\n')
        .find(|row| row.contains("public.customers"))
        .expect("customer row");
    assert!(!row.contains("> "), "cursor is on the first row: {row:?}");
}

#[test]
fn browser_renders_and_rasterizes() {
    let mut app = connected_app();
    app.update(support::key('j'));
    app.update(support::key('j'));

    let rendered = support::render_app(&app);
    let row = rendered
        .split('\n')
        .find(|row| row.contains("public.empty_table"))
        .expect("third table row");
    assert!(row.contains("> "), "cursor on the third row: {row:?}");

    let svg = support::render_app_svg(&app);
    assert!(svg.starts_with("<svg "), "{svg}");
    assert!(svg.contains("public.customers"), "{svg}");
    assert_eq!(support::render_app_png_dims(&app), (900, 540));
}
