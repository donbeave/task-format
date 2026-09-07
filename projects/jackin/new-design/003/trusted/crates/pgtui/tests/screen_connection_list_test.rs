//! Trusted TASK-002 gate: ConnectionList rendering (D-013, D-060).

mod support;

use pgtui::app::{App, Msg};

fn list_app(names: &[&str]) -> App {
    let connections: Vec<pgtui::store::SavedConnection> = names
        .iter()
        .copied()
        .map(support::saved_connection)
        .collect();
    let mut app = App::new();
    app.update(Msg::Connections(connections));
    app
}

fn lines(app: &App) -> Vec<String> {
    support::render_app(app)
        .split('\n')
        .map(String::from)
        .collect()
}

#[test]
fn empty_list_shows_hint_and_help() {
    let app = list_app(&[]);
    let rendered = support::render_app(&app);
    assert!(rendered.contains("pgtui - connections"), "{rendered}");
    assert!(
        rendered.contains("No saved connections. Press n to create one."),
        "{rendered}"
    );
    assert!(
        rendered.contains("j/k move  Enter connect  n new  q quit"),
        "{rendered}"
    );
    assert!(!rendered.contains("s3cr3t"), "no password on screen");
}

#[test]
fn rows_are_sorted_by_name_with_cursor() {
    let app = list_app(&["alpha", "beta"]);
    let rendered = support::render_app(&app);
    let lines = lines(&app);

    let alpha_at = rendered.find("alpha").expect("alpha row");
    let beta_at = rendered.find("beta").expect("beta row");
    assert!(alpha_at < beta_at, "rows in list order: {rendered}");

    let alpha_row = lines
        .iter()
        .find(|row| row.contains("alpha"))
        .expect("alpha row");
    assert!(
        alpha_row.contains("> alpha"),
        "cursor marker: {alpha_row:?}"
    );
    let beta_row = lines
        .iter()
        .find(|row| row.contains("beta"))
        .expect("beta row");
    assert!(
        !beta_row.contains("> "),
        "only the selected row is highlighted: {beta_row:?}"
    );

    assert!(
        rendered.contains("alice@example.com:5432/appdb"),
        "display DSN per row: {rendered}"
    );
    assert!(!rendered.contains("s3cr3t"), "never the password");
}

#[test]
fn svg_render_carries_text_and_rasterizes() {
    let app = list_app(&["alpha", "beta"]);
    let svg = support::render_app_svg(&app);
    assert!(svg.starts_with("<svg "), "{svg}");
    assert!(svg.contains("pgtui - connections"), "{svg}");
    assert!(svg.contains("beta"), "{svg}");
    assert_eq!(
        support::render_app_png_dims(&app),
        (900, 540),
        "100x30 cells at 9x18 px"
    );
}
