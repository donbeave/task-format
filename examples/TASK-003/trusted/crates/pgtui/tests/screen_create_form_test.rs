//! Trusted TASK-003 gate: CreateConnection rendering (D-032, D-060).

mod support;

use pgtui::app::{App, Effect, Field, Msg, Screen};

fn form_app() -> App {
    let mut app = App::new();
    app.update(Msg::Connections(vec![support::saved_connection("local")]));
    app.update(support::key('n'));
    app
}

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

fn fill(app: &mut App, name: &str, host: &str, port: &str, dbname: &str, user: &str) {
    focus_field(app, Field::Name);
    type_text(app, name);
    focus_field(app, Field::Host);
    type_text(app, host);
    focus_field(app, Field::Port);
    type_text(app, port);
    focus_field(app, Field::Database);
    type_text(app, dbname);
    focus_field(app, Field::User);
    type_text(app, user);
}

#[test]
fn blank_form_labels_and_focus_marker() {
    let app = form_app();
    let rendered = support::render_app(&app);
    assert!(rendered.contains(" New connection "), "{rendered}");
    assert!(rendered.contains("> Name:"), "focused field: {rendered}");
    for label in ["Host", "Port", "Database", "User", "Password"] {
        assert!(
            rendered.contains(&format!("  {label}:")),
            "{label}: {rendered}"
        );
    }
    assert!(
        rendered.contains("Tab next  Enter save  Esc cancel"),
        "{rendered}"
    );
}

#[test]
fn filled_form_masks_password() {
    let mut app = form_app();
    focus_field(&mut app, Field::Password);
    type_text(&mut app, "secret");

    let rendered = support::render_app(&app);
    assert!(rendered.contains("******"), "masked: {rendered}");
    assert!(
        !rendered.contains("secret"),
        "never the password: {rendered}"
    );
}

#[test]
fn saved_list_shows_new_row_selected_and_rasterizes() {
    let mut app = form_app();
    fill(&mut app, "prod", "db.example.com", "5432", "appdb", "alice");

    let effects = app.update(support::enter());
    let Effect::SaveConnection(_) = effects[0] else {
        panic!("expected SaveConnection, got {effects:?}");
    };

    let saved = support::saved_connection("prod");
    app.update(Msg::Saved(Ok(saved)));
    app.update(Msg::Connections(vec![
        support::saved_connection("alpha"),
        support::saved_connection("prod"),
    ]));

    assert_eq!(app.screen, Screen::ConnectionList);
    let rendered = support::render_app(&app);
    let row = rendered
        .split('\n')
        .find(|row| row.contains("prod"))
        .expect("saved row");
    assert!(row.contains("> prod"), "selected: {row:?}");

    let svg = support::render_app_svg(&app);
    assert!(svg.contains("prod"), "{svg}");
    assert_eq!(support::render_app_png_dims(&app), (900, 540));
}
