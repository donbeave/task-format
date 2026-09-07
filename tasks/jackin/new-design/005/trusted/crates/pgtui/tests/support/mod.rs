//! Planner-shipped test support (trusted material, never edited by an executor).
//!
//! Helpers grow per task as the application surface grows. Everything declared
//! in TASK-001, TASK-002 and TASK-004 keeps its exact signature for the whole
//! series.

#![allow(dead_code)]

pub mod fake_data;

use pgtui::app::{App, Msg};
use pgtui::db::ConnParams;
use pgtui::store::{ConnectionStore, NewConnection, SavedConnection};
use pgtui::ui;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Style;
use testcontainers_modules::postgres::Postgres;

/// Render terminal width in cells.
pub const COLUMNS: u16 = 100;
/// Render terminal height in cells.
pub const ROWS: u16 = 30;

// ---------------------------------------------------------------------------
// Buffer-level helpers (TASK-001, never change)
// ---------------------------------------------------------------------------

/// A `COLUMNS` x `ROWS` buffer with `lines` written from the top-left corner.
pub fn buffer(lines: &[&str]) -> Buffer {
    let mut buffer = Buffer::empty(Rect::new(0, 0, COLUMNS, ROWS));
    for (y, line) in lines.iter().enumerate() {
        buffer.set_string(0, u16::try_from(y).unwrap_or(ROWS), line, Style::new());
    }
    buffer
}

/// `pgtui::render::buffer_to_svg` on a hand-built buffer.
pub fn svg_of(lines: &[&str]) -> String {
    pgtui::render::buffer_to_svg(&buffer(lines))
}

/// Rasterizes `svg` and returns the PNG pixel dimensions.
pub fn png_dims(svg: &str) -> (u32, u32) {
    let png = pgtui::render::svg_to_png(svg);
    pgtui::render::png_dimensions(&png)
}

// ---------------------------------------------------------------------------
// App rendering (TASK-002)
// ---------------------------------------------------------------------------

/// Renders the app at 100x30 and returns the text: one line per row, trailing
/// spaces of every row trimmed.
pub fn render_app(app: &App) -> String {
    let mut buf = Buffer::empty(Rect::new(0, 0, COLUMNS, ROWS));
    ui::draw(app, &mut buf);
    pgtui::render::buffer_to_text(&buf)
}

/// Renders the app at 100x30 as a deterministic SVG document.
pub fn render_app_svg(app: &App) -> String {
    let mut buf = Buffer::empty(Rect::new(0, 0, COLUMNS, ROWS));
    ui::draw(app, &mut buf);
    pgtui::render::buffer_to_svg(&buf)
}

/// Rasterizes `render_app_svg(app)` and returns the PNG dimensions.
pub fn render_app_png_dims(app: &App) -> (u32, u32) {
    png_dims(&render_app_svg(app))
}

// ---------------------------------------------------------------------------
// Key constructors (TASK-002)
// ---------------------------------------------------------------------------

fn event(code: KeyCode, modifiers: KeyModifiers) -> Msg {
    Msg::Key(KeyEvent::new(code, modifiers))
}

/// The printable character `c`.
pub fn key(c: char) -> Msg {
    event(KeyCode::Char(c), KeyModifiers::empty())
}

/// A non-character key such as `KeyCode::Enter` or `KeyCode::Tab`.
pub fn key_code(code: KeyCode) -> Msg {
    event(code, KeyModifiers::empty())
}

/// `Ctrl+C`.
pub fn ctrl(c: char) -> Msg {
    event(KeyCode::Char(c), KeyModifiers::CONTROL)
}

/// `Enter`.
pub fn enter() -> Msg {
    key_code(KeyCode::Enter)
}

/// `Tab`.
pub fn tab() -> Msg {
    key_code(KeyCode::Tab)
}

/// `BackTab` (shift-tab).
pub fn back_tab() -> Msg {
    event(KeyCode::BackTab, KeyModifiers::SHIFT)
}

/// `Backspace`.
pub fn backspace() -> Msg {
    key_code(KeyCode::Backspace)
}

/// `Esc`.
pub fn esc() -> Msg {
    key_code(KeyCode::Esc)
}

/// `Down` arrow.
pub fn down() -> Msg {
    key_code(KeyCode::Down)
}

/// `Up` arrow.
pub fn up() -> Msg {
    key_code(KeyCode::Up)
}

/// `Left` arrow.
pub fn left() -> Msg {
    key_code(KeyCode::Left)
}

/// `Right` arrow.
pub fn right() -> Msg {
    key_code(KeyCode::Right)
}

// ---------------------------------------------------------------------------
// Store fixtures (TASK-002)
// ---------------------------------------------------------------------------

/// Opens (or creates) a store in a fresh temporary directory. The directory is
/// removed when the returned `TempDir` is dropped.
pub async fn temp_store() -> (tempfile::TempDir, ConnectionStore) {
    let dir = tempfile::tempdir().expect("trusted support: tempdir");
    let path = dir.path().join("connections.db");
    let store = ConnectionStore::open(&path)
        .await
        .expect("trusted support: store opens");
    (dir, store)
}

/// `NewConnection` for `name`: host `example.com`, port 5432, db `appdb`,
/// user `alice`, password `s3cr3t`.
pub fn new_connection(name: &str) -> NewConnection {
    NewConnection {
        name: name.to_string(),
        host: "example.com".to_string(),
        port: 5432,
        dbname: "appdb".to_string(),
        username: "alice".to_string(),
        password: "s3cr3t".to_string(),
    }
}

/// `SavedConnection` for `name` with the `new_connection` fields, id `1` and
/// `created_at` `2024-01-05T10:00:00Z`.
pub fn saved_connection(name: &str) -> SavedConnection {
    SavedConnection {
        id: 1,
        name: name.to_string(),
        host: "example.com".to_string(),
        port: 5432,
        dbname: "appdb".to_string(),
        username: "alice".to_string(),
        password: "s3cr3t".to_string(),
        created_at: "2024-01-05T10:00:00Z".to_string(),
    }
}

// ---------------------------------------------------------------------------
// PostgreSQL fixtures (TASK-004)
// ---------------------------------------------------------------------------

/// Starts a throwaway `postgres:16-alpine` with `tests/fixtures/seed.sql`
/// applied and returns the container plus the `ConnParams` for it. The
/// container is removed when the returned value is dropped.
pub async fn pg_container() -> (testcontainers::ContainerAsync<Postgres>, ConnParams) {
    use testcontainers::ImageExt;
    use testcontainers::runners::AsyncRunner;

    let seed = include_str!("../fixtures/seed.sql");
    let image = Postgres::default()
        .with_init_sql(seed.to_string().into_bytes())
        .with_tag("16-alpine".to_string());
    let container = image
        .start()
        .await
        .expect("trusted support: postgres container");
    let host = container.get_host().await.expect("container host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("mapped port");
    (
        container,
        ConnParams {
            host: host.to_string(),
            port,
            dbname: "postgres".to_string(),
            user: "postgres".to_string(),
            password: "postgres".to_string(),
        },
    )
}

/// `SavedConnection` for `name` carrying the container's host/port and the
/// `postgres` database, so an `Effect::Connect` can reach the container.
pub fn saved_connection_at(name: &str, params: &ConnParams) -> SavedConnection {
    SavedConnection {
        id: 1,
        name: name.to_string(),
        host: params.host.clone(),
        port: params.port,
        dbname: params.dbname.clone(),
        username: params.user.clone(),
        password: params.password.clone(),
        created_at: "2024-01-05T10:00:00Z".to_string(),
    }
}

/// A `--db` path that cannot be created: its parent is a regular file.
pub fn unwritable_db_path() -> tempfile::NamedTempFile {
    tempfile::NamedTempFile::new().expect("trusted support: temp file")
}
