# TASK-101 — fixed decisions (verbatim)

Planner-owned. Numbering is global across the pgtui task series; a D-id means the same thing in every task. Implement; do not reopen.

## Repository / crate layout

- **D-001 Workspace.** Cargo workspace, edition 2024, `resolver = "3"`. Single member `crates/pgtui` (lib + bin `pgtui`). Workspace `Cargo.toml` owns `[workspace.dependencies]` with every dependency pre-declared and pinned at fixture time; `crates/pgtui/Cargo.toml` uses `workspace = true`. Executors never add, remove, or bump a dependency. Pinned: `ratatui 0.30.2`, `crossterm 0.29.0`, `turso 0.7.2` (`default-features = false`), `tokio 1.53.1`, `tokio-postgres 0.7.18`, `clap 4.6.6`, `thiserror 2.0.20`, `directories 6.0.0`; dev: `insta 1.48.0`, `testcontainers 0.28.0`, `testcontainers-modules 0.15.0` (`postgres`), `assert_cmd`, `tempfile`, `resvg/usvg 0.48.1`, `tiny-skia 0.12`.
- **D-002 Module layout** (final; files may be empty stubs at baseline):

  ```text
  crates/pgtui/src/
    main.rs           CLI (clap): --db <path>; terminal setup/teardown; runtime loop; exit codes (D-040)
    lib.rs            pub mod app; keys; store; db; grid; ui; runtime; render
    app.rs            App state, Screen enum (D-010), Msg/Effect (D-011), App::update(Msg) -> Vec<Effect>
    keys.rs           KeyEvent -> Action mapping per screen (D-030..D-034)
    store/mod.rs      SavedConnection, NewConnection, StoreError, ConnectionStore (Turso)  (D-020..D-022)
    db/mod.rs         ConnParams, TableRef, ResultSet, Cell, DbError            (later tasks)
    db/postgres.rs    PgSession                                                   (later tasks)
    grid.rs           Grid                                                        (later tasks)
    runtime.rs        execute(Effect) -> Msg against ConnectionStore + Option<PgSession>
    render.rs         PLANNER, protected: Buffer -> text / SVG / PNG
    ui/mod.rs         draw(&mut Frame, &App); layout constants (D-060)
    ui/connection_list.rs  ui/create_form.rs  ui/browser.rs  ui/custom_sql.rs  ui/grid.rs  ui/status.rs
  ```

- **D-003 Test placement.** Trusted tests (planner) live in `crates/pgtui/tests/*_test.rs` and are protected. Executor-written tests are `#[cfg(test)] mod tests` inside the `src/` module they test; no executor-written file goes under `tests/`.
- **D-004 Toolchain/lint.** `rust-toolchain.toml` pins stable. Gate lint: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`. `#![allow(...)]`, `#[allow(clippy::...)]`, `todo!()`, `unimplemented!()`, `dbg!()` forbidden in `src/`.
- **D-005 `justfile`** recipes (fixture, read-only for executors): `test`, `test-unit` (`--lib`), `test-pg` (`--test 'pg_*'`), `lint`, `snap` (`cargo insta test --check`), `gallery`. Executors call `cargo` directly.

## App state machine

- **D-010 Screen enum** (exact):

  ```rust
  pub enum Screen { ConnectionList, CreateConnection, Browser, CustomSql }
  ```

  `App` fields (all pub for tests): `screen: Screen`, `connections: Vec<SavedConnection>`, `list_cursor: usize`, `form: CreateForm`, `session: Option<SessionView>` (`SessionView { name: String, tables: Vec<TableRef>, sidebar_cursor: usize, focus: Focus }`, `enum Focus { Sidebar, Grid }`), `grid: Option<Grid>`, `sql_input: String`, `sql_grid: Option<Grid>`, `status: Status` (`enum Status { Help, Info(String), Error(String) }`), `quit_requested: bool`. In this task `CreateForm`, `SessionView`, `TableRef`, `Grid` may be empty placeholder types; the field set is fixed.
- **D-011 Elm-style core.** `App::update(&mut self, msg: Msg) -> Vec<Effect>` is pure (no IO, no async). `runtime::execute(&mut Runtime, Effect) -> Option<Msg>` performs IO. Tests drive `App` with `Msg` values directly; snapshots never need a database.

  ```rust
  pub enum Msg {
      Key(crossterm::event::KeyEvent),
      Connections(Vec<SavedConnection>),
      Saved(Result<SavedConnection, StoreError>),
      Connected(Result<Vec<TableRef>, DbError>),
      QueryDone { kind: QueryKind, result: Result<ResultSet, DbError> },
      Disconnected,
  }
  pub enum QueryKind { Preview(TableRef), Custom }
  pub enum Effect {
      LoadConnections,
      SaveConnection(NewConnection),
      Connect(SavedConnection),      // runtime connects AND lists tables, replies Connected(..)
      Query { kind: QueryKind, sql: String },
      Disconnect,                    // replies Disconnected
      Quit,
  }
  ```

- **D-012 Runtime loop.** `main.rs`: parse CLI → open store (fail → exit 2) → enter raw mode/alternate screen → `App::update(Msg::Connections(store.list()))` → loop { draw; read key (blocking); effects = update(Key); for each effect: execute → feed reply Msg → collect further effects } until `Effect::Quit`. DB calls block the loop (no background tasks) — accepted; the app is single-user.
- **D-013 Status line.** Bottom row, height 1, full width. `Status::Help` shows the screen's key hint (D-060). `Info`/`Error` replace it until the next key press, then revert to `Help`. Error text is `error: <message>`; message is the first line of the underlying error, truncated to width. No modal dialogs.

## Storage (Turso)

- **D-020 Store engine.** `turso` (local SQLite file) via `turso::Builder::new_local(path).build().await` + `db.connect()`. Path precedence: `--db <path>` > `$PGTUI_DB` > `$XDG_DATA_HOME/pgtui/connections.db` (fallback `~/.local/share/pgtui/connections.db`). Parent dir created with `create_dir_all`; failure → exit 2.
- **D-021 Schema** (executed on every open, idempotent):

  ```sql
  CREATE TABLE IF NOT EXISTS connections (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT    NOT NULL UNIQUE,
    host       TEXT    NOT NULL,
    port       INTEGER NOT NULL,
    dbname     TEXT    NOT NULL,
    username   TEXT    NOT NULL,
    password   TEXT    NOT NULL,
    created_at TEXT    NOT NULL   -- RFC 3339 UTC, set by the store
  );
  ```

  Password stored plaintext. Keyring integration is out of scope.
- **D-022 Store API** (exact signatures; trusted tests compile against them):

  ```rust
  pub struct ConnectionStore { /* private */ }
  pub struct NewConnection { pub name: String, pub host: String, pub port: u16, pub dbname: String, pub username: String, pub password: String }
  pub struct SavedConnection { pub id: i64, pub name: String, pub host: String, pub port: u16, pub dbname: String, pub username: String, pub password: String, pub created_at: String }
  pub enum StoreError { Open(String), DuplicateName(String), Sql(String) }
  impl ConnectionStore {
      pub async fn open(path: &std::path::Path) -> Result<Self, StoreError>;
      pub async fn list(&self) -> Result<Vec<SavedConnection>, StoreError>;   // ORDER BY name ASC
      pub async fn insert(&self, new: NewConnection) -> Result<SavedConnection, StoreError>;
  }
  impl SavedConnection { pub fn display_dsn(&self) -> String; }  // "user@host:port/dbname", never the password
  ```

## Key bindings (single source: `keys.rs`)

- **D-030 Global.** `Ctrl+C` → `Effect::Disconnect` (if connected) then `Effect::Quit`, from every screen. No other global key.
- **D-031 ConnectionList.** `j`/`Down` cursor +1, `k`/`Up` cursor −1 (clamped, no wrap); `Enter` → `Effect::Connect(selected)`; `n` → `Screen::CreateConnection` with blank form; `q` → `Effect::Quit`. Empty list: `Enter` is a no-op. (In TASK-101 `Enter` and `n` are no-ops; later tasks wire them.)

## Exit codes and errors

- **D-040 Exit codes.** `0` normal quit (`q`, `Ctrl+C`). `2` usage/config: bad CLI args, store open failure, cannot create data dir — message on stderr as `error: <msg>`, before entering raw mode. `1` unexpected runtime failure (terminal IO error, panic) — terminal restored by a panic hook first.
- **D-041 Error surfacing.** Every `Err` reaching `App::update` becomes `Status::Error` (D-013). No error is swallowed; no `unwrap()`/`expect()` on IO results in `src/` outside `main.rs`'s terminal setup.

## Layout (deterministic; snapshots depend on it)

- **D-060 Frame.** Snapshot terminal 100×30. Root layout: `[Min(0)] + [Length(1)]` → body + status line. Borders: `Borders::ALL`, plain. Titles wrapped in single spaces.
  - ConnectionList: body = one block, title ` pgtui - connections `, `List` rows `<name>  <display_dsn>`, `highlight_symbol` `> `; empty list shows the single line `No saved connections. Press n to create one.`. Help: `j/k move  Enter connect  n new  q quit`.
- **D-061 Rendering entry point.** `ui::draw(frame: &mut Frame, app: &App)`; tests render with `ratatui::backend::TestBackend::new(100, 30)`.

## Test infrastructure (planner-shipped, protected)

- **D-070 `tests/support/mod.rs`** provides: `render_text(&App) -> String` (TestBackend buffer → lines, trailing spaces trimmed); `render_svg(&App) -> String` (deterministic string); `svg_to_png(&str) -> Vec<u8>`; `key(char)`, `key_code(KeyCode)`, `ctrl(char)` constructors; `temp_store() -> (TempDir, ConnectionStore)`; `pg_container()` (later tasks); `fake_data`.
- **D-071 Snapshot policy.** Text snapshots via `insta::assert_snapshot!("<name>", render_text(&app))`, SVG via `insta::assert_snapshot!("<name>__svg", render_svg(&app))`. Names for this task: `screen__connection_list_empty`, `screen__connection_list_two`. All `.snap` files are shipped by the planner and protected. Gate runs tests with `INSTA_UPDATE=no INSTA_FORCE_PASS=0`; `*.snap.new` anywhere fails the gate.
