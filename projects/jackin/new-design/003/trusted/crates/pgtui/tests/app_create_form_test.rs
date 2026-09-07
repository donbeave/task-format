//! Trusted TASK-003 gate: CreateConnection form editing and validation (D-032).

mod support;

use pgtui::app::{App, CreateForm, Field, Msg, Screen};

fn list_app() -> App {
    let mut app = App::new();
    app.update(Msg::Connections(vec![support::saved_connection("local")]));
    app
}

fn form_app() -> App {
    let mut app = list_app();
    app.update(support::key('n'));
    assert_eq!(app.screen, Screen::CreateConnection);
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
fn n_opens_blank_form() {
    let app = form_app();
    assert_eq!(app.form, CreateForm::blank());
    assert_eq!(app.form.focus, Field::Name);
    assert_eq!(app.form.name, "");
    assert_eq!(app.connections.len(), 1, "list state is kept");
}

#[test]
fn tab_cycles_fields_forward() {
    let mut app = form_app();
    let order = [
        Field::Host,
        Field::Port,
        Field::Database,
        Field::User,
        Field::Password,
        Field::Name,
    ];
    for expected in order {
        app.update(support::tab());
        assert_eq!(app.form.focus, expected, "after Tab");
    }
}

#[test]
fn backtab_moves_previous() {
    let mut app = form_app();
    app.update(support::back_tab());
    assert_eq!(app.form.focus, Field::Password, "wraps backwards");

    app.update(support::tab());
    assert_eq!(app.form.focus, Field::Name);
}

#[test]
fn printable_chars_append_to_focused_field() {
    let mut app = form_app();
    type_text(&mut app, "prod");
    assert_eq!(app.form.name, "prod");
    assert_eq!(app.form.host, "", "other fields untouched");
}

#[test]
fn backspace_pops_char() {
    let mut app = form_app();
    type_text(&mut app, "prod");
    app.update(support::backspace());
    assert_eq!(app.form.name, "pro");

    app.update(support::backspace());
    app.update(support::backspace());
    app.update(support::backspace());
    app.update(support::backspace());
    assert_eq!(app.form.name, "", "backspace on empty field is a no-op");
}

#[test]
fn port_ignores_non_digits() {
    let mut app = form_app();
    focus_field(&mut app, Field::Port);
    type_text(&mut app, "5x43y2");
    assert_eq!(app.form.port, "5432", "only digits are appended");

    type_text(&mut app, "9");
    assert_eq!(app.form.port, "54329");
}

#[test]
fn enter_with_missing_name_reports_name_required() {
    let mut app = form_app();
    fill(&mut app, "", "db.example.com", "5432", "appdb", "alice");

    let effects = app.update(support::enter());
    assert!(
        effects.is_empty(),
        "invalid form emits nothing: {effects:?}"
    );
    assert_eq!(app.screen, Screen::CreateConnection);
    assert_eq!(
        app.status,
        pgtui::app::Status::Error("name is required".to_string())
    );
}

#[test]
fn enter_with_bad_port_reports_port_error() {
    let mut app = form_app();
    fill(&mut app, "prod", "db.example.com", "0", "appdb", "alice");

    app.update(support::enter());
    assert_eq!(app.screen, Screen::CreateConnection);
    assert_eq!(
        app.status,
        pgtui::app::Status::Error("port must be 1-65535".to_string())
    );
}

#[test]
fn esc_returns_to_list_and_discards() {
    let mut app = form_app();
    type_text(&mut app, "prod");
    app.update(support::esc());

    assert_eq!(app.screen, Screen::ConnectionList);
    assert_eq!(app.connections.len(), 1, "nothing saved");

    app.update(support::key('n'));
    assert_eq!(app.screen, Screen::CreateConnection);
    assert_eq!(app.form, CreateForm::blank(), "form discarded");
}
