# TASK-002 — fixed decisions (verbatim)

Planner-owned and self-contained: every D-* in force for this task is written here in full. Numbering is global across the pgtui series; a D-id means the same thing in every task. Implement; do not reopen. Anything not covered here that changes public behaviour, architecture, data or security posture is `NEEDS_REPLAN`, not executor discretion.

In force for TASK-002: D-001..D-005, D-010..D-013, D-020..D-022, D-030..D-031, D-040..D-041, D-060..D-061, D-070..D-071.

## Repository, workspace, exact pins

- **D-001 Workspace and exact pins.** Cargo workspace, `edition = "2024"`, `resolver = "3"`, single member `crates/pgtui` (lib + bins `pgtui`, `gallery`). The workspace `Cargo.toml` owns `[workspace.dependencies]` and `[workspace.lints.rust] unsafe_code = "forbid"`; the member manifest uses `workspace = true` for every runtime dependency. Executors never add, remove, or bump a dependency. Exact pins, all with `=`:
  runtime: `ratatui 0.30.2`, `crossterm 0.29.0`, `turso 0.7.2` (`default-features = false`), `tokio 1.53.1` (features `rt`, `macros`, `time`, `sync`), `tokio-postgres 0.7.18`, `clap 4.6.6` (feature `derive`), `thiserror 2.0.20`, `directories 6.0.0`, `resvg 0.48.1` (used only by the planner-shipped `render.rs`);
  dev: `tempfile 3.27.0`, `testcontainers 0.27.3`, `testcontainers-modules 0.15.0` (feature `postgres`), `nix 0.30.1` (feature `term`).
  `Cargo.lock` is committed and stays in sync. There is no `insta` and no `assert_cmd`: verification material asserts behaviour, never snapshots (D-071). `crates/pgtui/src/render.rs`, `crates/pgtui/src/fonts/` and everything under `crates/pgtui/tests/` are placed on the run base commit by the harness (trusted overlay, D-070); they are not created by this task, are outside the scope whitelist (D-003), and the manifests must still compile them.

- **D-002 Module layout** (final shape; modules arrive with their task and may be empty stubs until then):

  ```text
  crates/pgtui/src/
    main.rs           CLI (clap): --db <path>; terminal setup/teardown; runtime loop; exit codes (D-040)
    lib.rs            TASK-001: exactly `pub mod render;`; later tasks add the modules below
    app.rs            App state, Screen enum (D-010), Msg/Effect (D-011), App::update(Msg) -> Vec<Effect>
    keys.rs           KeyEvent -> per-screen handling (D-030..D-034)
    store/mod.rs      SavedConnection, NewConnection, StoreError, ConnectionStore (Turso) (D-020..D-022)
    db/mod.rs         ConnParams, TableRef, ResultSet, Cell, DbError, QueryOutcome (D-026)
    db/postgres.rs    PgSession (D-023..D-025)
    grid.rs           Grid (D-050..D-053)
    runtime.rs        async execute(Effect) -> Option<Msg> against ConnectionStore + Option<PgSession>
    render.rs         PLANNER-SHIPPED, protected: Buffer -> text / SVG / PNG (resvg + bundled font)
    fonts/            PLANNER-SHIPPED, protected: DejaVuSansMono.ttf, include_bytes!-ed by render.rs
    ui/mod.rs         draw(app: &App, buf: &mut Buffer); layout constants (D-060)
    ui/connection_list.rs  ui/create_form.rs  ui/browser.rs  ui/custom_sql.rs  ui/grid.rs  ui/status.rs
  crates/pgtui/src/bin/gallery.rs   gallery binary (D-080), declared in the manifest from TASK-001
  ```

  `render.rs` and `fonts/` are never created, edited, or deleted by the executor; they ship with the trusted overlay and are excluded from the scope whitelist.

- **D-003 Test placement and trusted material.** Trusted tests (planner-shipped) live in `crates/pgtui/tests/*_test.rs` with `crates/pgtui/tests/support/mod.rs` and `crates/pgtui/tests/fixtures/seed.sql`; they are protected, are outside `expected_paths`/`allowed_globs`, and must not be created, edited, or deleted. Executor-written tests are `#[cfg(test)] mod tests` inside the `src/` module they test; no executor-written file goes under `tests/`.

- **D-004 Toolchain and lint gate.** `rust-toolchain.toml` pins `1.98.0`; `rustfmt.toml` sets `edition = "2024"`; `.cargo/config.toml` sets `TESTCONTAINERS_COMMAND = "remove"`; `.gitignore` lists `target/`. Gate lint: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`. Forbidden in `crates/pgtui/src/`: `#![allow(...)]`, `#[allow(clippy::...)]`, `#[allow(dead_code)]`, `#[allow(unused)]`, `todo!()`, `unimplemented!()`, `dbg!()`. Forbidden in `crates/pgtui/tests/`: `#[ignore]`.

- **D-005 No repository task runner.** No `justfile`, `Makefile`, `xtask`, or similar is created; executors call `cargo` and `taskfmt` directly. The repository `README.md` (with its `## Screens` section) is written by TASK-007 only; no earlier task creates it.

## App state machine

- **D-010 Screen enum and `App` fields** (exact, fixed from TASK-002 on):

  ```rust
  pub enum Screen { ConnectionList, CreateConnection, Browser, CustomSql }
  ```

  `App` fields (all pub for tests): `screen: Screen`, `connections: Vec<SavedConnection>`, `list_cursor: usize`, `form: CreateForm`, `session: Option<SessionView>`, `grid: Option<Grid>`, `sql_input: String`, `sql_grid: Option<Grid>`, `status: Status` (`enum Status { Help, Info(String), Error(String) }`), `quit_requested: bool`, plus private runtime-only state the task needs (e.g. a pending preview selector).
  `CreateForm` (TASK-003, exact): `pub struct CreateForm { pub name: String, pub host: String, pub port: String, pub dbname: String, pub username: String, pub password: String, pub focus: Field }`, `pub enum Field { Name, Host, Port, Database, User, Password }` (order = D-032), `impl CreateForm { pub fn blank() -> Self; pub fn validate(&self) -> Result<NewConnection, String>; }`.
  `SessionView` (TASK-004, exact): `pub struct SessionView { pub name: String, pub tables: Vec<TableRef>, pub sidebar_cursor: usize, pub focus: Focus }`, `pub enum Focus { Sidebar, Grid }`.
  Placeholder rule: in a task that has not yet reached the decision defining one of these types, it may be an empty placeholder struct (unit or `PhantomData`), but the `App` field set above and the variant set of every enum are already fixed and must not be extended later.

- **D-011 Elm-style core.** `App::update(&mut self, msg: Msg) -> Vec<Effect>` is pure: no IO, no await. `pub async fn runtime::execute(&mut Runtime, Effect) -> Option<Msg>` performs the IO and is awaited by the runtime loop. Tests drive `App` with `Msg` values directly; none of the behaviour tests need a database except the `pg_*` ones.

  ```rust
  pub enum Msg {
      Key(crossterm::event::KeyEvent),
      Connections(Vec<SavedConnection>),
      Saved(Result<SavedConnection, StoreError>),
      Connected(Result<Vec<TableRef>, DbError>),
      QueryDone { kind: QueryKind, result: Result<QueryOutcome, DbError> },
      Disconnected,
  }
  pub enum QueryKind { Preview(TableRef), Custom }
  pub enum QueryOutcome { Rows(ResultSet), Affected(u64) }
  pub enum Effect {
      LoadConnections,
      SaveConnection(NewConnection),
      Connect(SavedConnection),   // runtime connects AND lists tables, replies Connected(..)
      Query { kind: QueryKind, sql: String },
      Disconnect,                 // replies Disconnected
      Quit,
  }
  ```

  `QueryKind`, `QueryOutcome`, `TableRef`, `ResultSet` and `DbError` may be placeholders until their task lands (D-010); the variant sets above are fixed.

- **D-012 Runtime loop.** `main.rs`: parse CLI -> open store (fail -> exit 2) -> enter raw mode/alternate screen -> `App::update(Msg::Connections(store.list().await))` -> loop { draw; read key (blocking); `let effects = app.update(Msg::Key(key))`; for each effect: `runtime::execute(..).await` -> feed the reply `Msg` back into `App::update` and collect the further effects it produces } until `Effect::Quit`. A current-thread `tokio` runtime drives the async store/PG calls; DB calls block the loop (no background tasks) — accepted, the app is single-user. `std::process::exit` is called only from `main.rs`.

- **D-013 Status line.** Bottom row, height 1, full width. `Status::Help` shows the screen's key hint (D-060). `Info`/`Error` replace it until the next key press, then revert to `Help`. Error text is `error: <message>`, message being the first line of the underlying error, truncated to width. No modal dialogs.

## Storage (Turso)

- **D-020 Store engine.** `turso` (local SQLite file) via `turso::Builder::new_local(path).build().await` + `db.connect()`. Path precedence: `--db <path>` > `$PGTUI_DB` > `$XDG_DATA_HOME/pgtui/connections.db` (fallback `~/.local/share/pgtui/connections.db`). Parent directory created with `create_dir_all`; failure to open or create -> exit 2 (D-040).

- **D-021 Schema** (executed on every open, idempotent): table `connections(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, host TEXT NOT NULL, port INTEGER NOT NULL, dbname TEXT NOT NULL, username TEXT NOT NULL, password TEXT NOT NULL, created_at TEXT NOT NULL)`. `created_at` is RFC 3339 UTC seconds (`YYYY-MM-DDTHH:MM:SSZ`), set by the store. Password stored plaintext; keyring integration is out of scope.

- **D-022 Store API** (exact signatures; trusted tests compile against them):

  ```rust
  pub struct ConnectionStore { /* private */ }
  pub struct NewConnection { pub name: String, pub host: String, pub port: u16, pub dbname: String, pub username: String, pub password: String }
  pub struct SavedConnection { pub id: i64, pub name: String, pub host: String, pub port: u16, pub dbname: String, pub username: String, pub password: String, pub created_at: String }
  #[derive(Debug, Clone)]
  pub enum StoreError { Open(String), DuplicateName(String), Sql(String) }
  impl ConnectionStore {
      pub async fn open(path: &std::path::Path) -> Result<Self, StoreError>;
      pub async fn list(&self) -> Result<Vec<SavedConnection>, StoreError>;   // ORDER BY name ASC
      pub async fn insert(&self, new: NewConnection) -> Result<SavedConnection, StoreError>;
  }
  impl SavedConnection { pub fn display_dsn(&self) -> String; }  // "user@host:port/dbname", never the password
  ```

## Key bindings (single source: `keys.rs`)

- **D-030 Global.** `Ctrl+C` -> `Effect::Disconnect` (if connected) then `Effect::Quit`, from every screen. No other global key.
- **D-031 ConnectionList.** `j`/`Down` cursor +1, `k`/`Up` cursor -1 (clamped, no wrap); `Enter` -> `Effect::Connect(selected)` (no-op until TASK-004 wires it); `n` -> `Screen::CreateConnection` with blank form; `q` -> `Effect::Quit`. Empty list: `Enter` is a no-op.

## Exit codes and errors

- **D-040 Exit codes.** `0` normal quit (`q`, `Ctrl+C`). `2` usage/config: bad CLI args, store open failure, cannot create data dir, unwritable output directory — message on stderr as `error: <msg>`, before entering raw mode. `1` unexpected runtime failure (terminal IO error, panic) — terminal restored by a panic hook first. From TASK-001 until its task replaces it, both binaries are stubs that print `error: not implemented` (`gallery`: usage) on stderr and exit 2.

- **D-041 Error surfacing.** Every `Err` reaching `App::update` becomes `Status::Error` (D-013). No error is swallowed; no `unwrap()`/`expect()` on IO results in `src/` outside `main.rs`'s terminal setup.

## Layout (deterministic; behaviour tests depend on it)

- **D-060 Frame.** Render terminal 100x30 cells (`CELL_WIDTH = 9`, `CELL_HEIGHT = 18`, so 900x540 px). Root layout: `[Min(0)] + [Length(1)]` -> body + status line. Borders: `Borders::ALL`, plain. Titles wrapped in single spaces. The focused pane's title gets suffix `*` inside the brackets (e.g. ` Tables (4)* `).
  - ConnectionList: body = one block, title ` pgtui - connections `, `List` rows `<name>  <display_dsn>`, `highlight_symbol` `> `; empty list shows the single line `No saved connections. Press n to create one.`. Help: `j/k move  Enter connect  n new  q quit`.

- **D-061 Rendering entry point.** `pub fn draw(app: &App, buffer: &mut ratatui::buffer::Buffer)` in `ui/mod.rs` is the only entry point; every screen draws through it. `app.rs`, `store/`, `db/` and `runtime.rs` never import `ui`. Trusted tests build the buffer themselves (`Buffer::empty(Rect::new(0, 0, 100, 30))`), so no `Frame`, no `TestBackend`, and no terminal is needed.

## Trusted verification material (planner-shipped, protected)

- **D-070 `tests/support/mod.rs`.** Imported by every trusted test as `mod support;`. Provides the 100x30 render helpers `render_app(&App) -> String` (`ui::draw` into a fresh buffer, then `pgtui::render::buffer_to_text`), `render_app_svg(&App) -> String`, `render_app_png_dims(&App) -> (u32, u32)`, key constructors `key(char)`, `key_code(KeyCode)`, `ctrl(char)`, `enter()`, `tab()`, `back_tab()`, `backspace()`, `esc()`, `down()`, `up()`, `left()`, `right()`, `temp_store() -> (TempDir, ConnectionStore)`, `new_connection(name)`, `saved_connection(name)`, `unwritable_db_path()`; from TASK-004 also `fake_data` and `pg_container() -> (ContainerAsync<Postgres>, ConnParams)` starting `postgres:16-alpine` with `fixtures/seed.sql` applied via `with_init_sql`. The helper set grows per task; earlier tasks' tests keep compiling unchanged.

- **D-071 Behavioural verification, not golden snapshots.** Trusted tests assert behaviour only: substring presence, line ordering and counts on rendered text, store roundtrips, exit codes, and PNG dimensions read from the IHDR chunk. There are no insta snapshots and no golden bytes: any correct implementation passes, and the material stays valid when formatting or styling changes that do not alter behaviour. Every trusted test fails (compile error or assertion) while the feature it covers is absent. No `*.snap` files exist anywhere in the repository.
