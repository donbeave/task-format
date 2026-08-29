# TASK-007 — fixed decisions (verbatim)

Planner-owned and self-contained: every D-* in force for this task is written here in full. Numbering is global across the pgtui series; a D-id means the same thing in every task. Implement; do not reopen. Anything not covered here that changes public behaviour, architecture, data or security posture is `NEEDS_REPLAN`, not executor discretion.

In force for TASK-007: D-001..D-005, D-010..D-013, D-020..D-026, D-030..D-034, D-040..D-042, D-050..D-053, D-060..D-061, D-070..D-072, D-080.

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

## PostgreSQL

- **D-023 Client + protocol.** `tokio-postgres` 0.7.18, **simple-query protocol** (`Client::simple_query`) for every statement the app runs. Simple-query returns every cell as text (`Option<&str>`), so preview and custom SQL need no per-type decoding, NULL is `None`, and unknown types cannot fail decoding. `SimpleQueryMessage::Row(row)`: `row.columns()[i].name()`, `row.get(i) -> Option<&str>`. `query`, `query_one`, `query_raw`, `prepare` and `execute` are forbidden in `crates/pgtui/src/db/`.

- **D-024 Connection config.** Built from fields, never typed by the user: `host=<host> port=<port> dbname=<dbname> user=<username> password=<password> application_name=pgtui` (tokio-postgres key/value config, `NoTls`). No sslmode, no URL parsing. Connect timeout 5 s (`tokio::time::timeout` around `config.connect(NoTls)`); elapsed -> `DbError::Timeout`, other errors -> `DbError::Connect(<message>)`. The connection future is `tokio::spawn`ed and its `JoinHandle` kept in `PgSession`. Failure is non-fatal: `Msg::Connected(Err)` -> status `error: ...`, screen stays `ConnectionList`.
  Disconnect (TASK-007): the runtime removes the `PgSession`, drops the `Client`, and awaits the spawned connection's `JoinHandle` so the socket is closed before `Msg::Disconnected` is returned. `application_name=pgtui` exists so `pg_disconnect_test` can count backends in `pg_stat_activity`.

- **D-025 Queries** (exact SQL; identifiers double-quoted, embedded `"` doubled by `quote_ident`):
  - tables: `SELECT table_schema, table_name FROM information_schema.tables WHERE table_type = 'BASE TABLE' AND table_schema NOT IN ('pg_catalog', 'information_schema') ORDER BY table_schema, table_name`
  - preview (TASK-005): `SELECT * FROM "<schema>"."<table>" LIMIT 500` — `PREVIEW_LIMIT: usize = 500` const in `db/mod.rs`; no `ORDER BY`; row order is heap order and `pg_preview_test::preview_matches_fake` compares real rows to fake rows sorted by primary key inside the test. `pg_preview_test::limit_applied_on_600_row_table` creates its own 600-row table inside the test, not in `seed.sql`.
  - custom SQL (TASK-006): sent verbatim after trimming whitespace and one trailing `;`. More than one row description -> `DbError::MultiStatement`. Non-row results (`CommandComplete`) -> `QueryOutcome::Affected(n)` -> `Status::Info("ok: <n> rows affected")`, `sql_grid = None`. Row results are capped client-side at `PREVIEW_LIMIT`; when capped, `Status::Info("showing first 500 rows")`. Everything goes through `Client::simple_query` (D-023).
  - `PREVIEW_LIMIT` is declared in `db/mod.rs` from TASK-004 on.

- **D-026 Types** (`db/mod.rs`, exact from TASK-004 on):

  ```rust
  pub const PREVIEW_LIMIT: usize = 500;
  pub struct ConnParams { pub host: String, pub port: u16, pub dbname: String, pub user: String, pub password: String }
  impl From<&SavedConnection> for ConnParams { /* field copy */ }
  pub struct TableRef { pub schema: String, pub name: String }   // Display = "schema.name"
  pub struct ResultSet { pub columns: Vec<String>, pub rows: Vec<Vec<Cell>> }
  pub enum Cell { Null, Text(String) }                            // Null renders as NULL
  pub enum QueryOutcome { Rows(ResultSet), Affected(u64) }
  #[derive(Debug, Clone)]
  pub enum DbError { Connect(String), Timeout, Query(String), MultiStatement }   // Display: first line, no prefix
  pub fn quote_ident(s: &str) -> String;   // "\"" + s.replace('"', "\"\"") + "\""
  ```

  `db/postgres.rs`: `pub struct PgSession { /* private */ }`, `impl PgSession { pub async fn connect(p: &ConnParams) -> Result<Self, DbError>; pub async fn list_tables(&self) -> Result<Vec<TableRef>, DbError>; pub async fn query(&self, sql: &str) -> Result<QueryOutcome, DbError>; }`.

## Key bindings (single source: `keys.rs`)

- **D-030 Global.** `Ctrl+C` -> `Effect::Disconnect` (if connected) then `Effect::Quit`, from every screen. No other global key.
- **D-031 ConnectionList.** `j`/`Down` cursor +1, `k`/`Up` cursor -1 (clamped, no wrap); `Enter` -> `Effect::Connect(selected)`; `n` -> `Screen::CreateConnection` with blank form; `q` -> `Effect::Quit`. Empty list: `Enter` is a no-op.
- **D-032 CreateConnection.** Fields in order: Name, Host, Port, Database, User, Password. `Tab` next field, `BackTab` previous (wrap). Printable chars append to the focused field; `Backspace` pops. Port accepts digits only (other chars ignored). `Enter` validates: all fields non-empty except Password (may be empty), Port parses `u16` >= 1; on failure `Status::Error` with the first failing field (`error: <field> is required` / `error: port must be 1-65535`; `<field>` is the label lowercased, so an empty Name gives exactly `error: name is required`, lowercase like `error: name already exists` below; labels as displayed: `Name`, `Host`, `Port`, `Database`, `User` — never the struct field names `dbname`/`username`), stay on form. On success -> `Effect::SaveConnection`; `Msg::Saved(Ok)` -> reload list (`Effect::LoadConnections`), `Screen::ConnectionList`, cursor on the new row; `Msg::Saved(Err(DuplicateName))` -> `error: name already exists`, stay on form. `Esc` -> `Screen::ConnectionList`, form discarded.
- **D-033 Browser.** `Tab` toggles `Focus::Sidebar` <-> `Focus::Grid` (Grid only if `grid.is_some()`; no-op otherwise). Sidebar focus: `j`/`k`/`Up`/`Down` move (clamped, no wrap); `Enter` -> `Effect::Query { kind: Preview(table), sql }` with the exact D-025 preview SQL. Grid focus: `h`/`Left`, `l`/`Right` move the column cursor (clamped); `j`/`k`/`Up`/`Down` move the row cursor (clamped); `s` cycles sort on the cursor column (D-052). `x` -> `Screen::CustomSql` (keeps session and grid). `d` -> `Effect::Disconnect`. `Ctrl+C` (D-030) works from Browser.
- **D-034 CustomSql.** Printable chars append to `sql_input`, `Backspace` pops, `Enter` -> `Effect::Query { kind: Custom, sql }` (empty input -> no-op). `Esc` -> `Screen::Browser` (session, grid and input retained). Result grid: `Up`/`Down` move the row cursor only; **no sort, no column cursor**; `sort` stays `None`, `col_cursor` stays `0`. Because letters type into the input there is no `d`/`q` here; disconnect via `Esc` then `d`, exit via `Ctrl+C` (D-030). Entry: `x` in Browser (D-033).

## Exit codes and errors

- **D-040 Exit codes.** `0` normal quit (`q`, `Ctrl+C`). `2` usage/config: bad CLI args, store open failure, cannot create data dir, unwritable output directory — message on stderr as `error: <msg>`, before entering raw mode. `1` unexpected runtime failure (terminal IO error, panic) — terminal restored by a panic hook first. From TASK-001 until its task replaces it, both binaries are stubs that print `error: not implemented` (`gallery`: usage) on stderr and exit 2.

- **D-041 Error surfacing.** Every `Err` reaching `App::update` becomes `Status::Error` (D-013). No error is swallowed; no `unwrap()`/`expect()` on IO results in `src/` outside `main.rs`'s terminal setup.

- **D-042 Non-interactive invocation** (in force from TASK-002 on; D-040 alone governs TASK-001, where both binaries are still stubs). `pgtui` is a full-screen program, every automated oracle launches it with no controlling terminal, and D-040's table has no row for that case — so the contract is stated here rather than derived from whichever library call happens to fail first.
  - **When the check happens.** An invocation is *interactive* when both stdin and stdout are connected to a terminal. `pgtui` decides this once, after clap has parsed the arguments and after the store has been opened — the boundary D-040 already names, immediately before entering raw mode. `--version` and `--help` therefore still exit 0, and a store failure still reports the store's own `error: <msg>` and exits 2 (D-020, D-040), never the message below.
  - **What it does.** When the invocation is not interactive, `pgtui` writes exactly `error: no terminal: pgtui requires an interactive terminal` and a newline to stderr, writes nothing to stdout, does not enter raw mode or the alternate screen, and exits 2. This is D-040's usage/config row, not its runtime-failure row: the absence of a terminal is a property of the invocation, not an unexpected failure, and it is decided before any terminal state is touched. The mechanism is not decided here: how `pgtui` establishes that stdin and stdout are terminals is the implementer's choice. This clause fixes the observable bytes — the message, the streams, the exit code and the point in the sequence at which the check is made — and nothing else.
  - **What it is not.** The TASK-001 stub message D-040 assigns to `pgtui` is retired the moment `pgtui`'s own task lands (TASK-002 R-005). The message above is a different string on purpose; reusing the retired wording for this path would reinstate the behaviour R-005 removes, so it must not appear in `crates/pgtui/src/main.rs` from TASK-002 on. `gallery` is never interactive, never performs this check, and keeps the exit codes its own decisions give it.
  - **Consequence for the trusted material.** `crates/pgtui/tests/skeleton_test.rs` is TASK-001's oracle: `pgtui_stub_exits_2` asserts the stub message this clause retires, and `gallery_stub_exits_2` asserts the `gallery` stub that the gallery task replaces in turn. From TASK-002 on every package's gate still runs that target with `pgtui_stub_exits_2` skipped by name, and the gallery task's own package skips `gallery_stub_exits_2` as well once its decisions replace that stub; the file's other two tests, which assert the `render` text, SVG and PNG pipeline, keep running in every package. The file itself is unchanged and stays byte-identical in every package carrying it; each of its assertions is in force only where its own subject is.
  - **Supersession.** This clause is `pgtui`-specific and self-contained; nothing else in this file depends on its internals. When a format-level contract for the invocation environment lands, delete this clause whole and cite the new one from D-040 and from TASK-002 R-005.

## Grid (TASK-005)

- **D-050 Grid model.**

  ```rust
  pub struct Grid { pub columns: Vec<String>, rows: Vec<Vec<Cell>>, pub col_cursor: usize, pub row_cursor: usize, pub sort: Option<SortState> }
  pub struct SortState { pub column: usize, pub dir: SortDir }
  pub enum SortDir { Asc, Desc }
  ```

  `Grid::from(ResultSet)` keeps the result-set order; `Grid::visible_rows(&self) -> Vec<&Vec<Cell>>` returns the sorted view (original order when `sort == None`); the original row order is never mutated. `sql_grid` reuses this type; its `sort` stays `None` and `col_cursor` stays `0` (D-034).

- **D-051 Comparison.** Per column: if every non-null cell parses as `f64` -> numeric compare; else byte-wise `str` compare. Stable sort (`sort_by`); ties keep the original order.

- **D-052 Null placement + cycling.** Asc: NULLs last. Desc: NULLs first (PostgreSQL semantics). `s` on the cursor column: `None -> Asc -> Desc -> None`; `s` on a different column starts at `Asc` and replaces the previous sort. Sort state resets when a new preview or result set loads.

- **D-053 Sort is client-side.** Over the fetched <= 500 rows; no re-query.

## Layout (deterministic; behaviour tests depend on it)

- **D-060 Frame.** Render terminal 100x30 cells (`CELL_WIDTH = 9`, `CELL_HEIGHT = 18`, so 900x540 px). Root layout: `[Min(0)] + [Length(1)]` -> body + status line. Borders: `Borders::ALL`, plain. Titles wrapped in single spaces. The focused pane's title gets suffix `*` inside the brackets (e.g. ` Tables (4)* `).
  - ConnectionList: body = one block, title ` pgtui - connections `, `List` rows `<name>  <display_dsn>`, `highlight_symbol` `> `; empty list shows the single line `No saved connections. Press n to create one.`. Help: `j/k move  Enter connect  n new  q quit`.
  - CreateConnection: block title ` New connection `; six lines `  <Label>: <value>` (labels `Name`, `Host`, `Port`, `Database`, `User`, `Password`), focused line prefixed `> ` instead of two spaces, Password shown as `*` per char. Help: `Tab next  Enter save  Esc cancel`.
  - Browser: horizontal `[Length(30)] + [Min(0)]`. Sidebar block title ` Tables (<n>) `, rows `schema.name`, `highlight_symbol` `> `. Main block title ` <connection name> ` while `grid.is_none()` (body empty), else ` <schema.table>  <rows> rows  limit 500 `. Help: `Tab focus  j/k move  Enter preview  h/l col  s sort  x sql  d disconnect`.
  - Grid (shared widget): ratatui `Table`; header row = column names; cursor column header wrapped as `[name]`; sorted column header suffixed ` ^` (asc) or ` v` (desc), e.g. `[balance ^]`; column width = `clamp(max(len(header)+4, max cell len), 4, 24)`; `highlight_symbol` `> ` on the cursor row; `Cell::Null` renders as `NULL`. The CustomSql result grid uses the same widget with **no `[ ]` on any header and no sort markers**.
  - CustomSql: vertical `[Length(3)] + [Min(0)]`. Input block title ` SQL `, content `> <sql_input>`; results block title ` Results ` while empty with body line `Type a query and press Enter`, after a result ` Results  <rows> rows `. Help: `Enter run  Up/Down rows  Esc back  Ctrl+C quit`.

- **D-061 Rendering entry point.** `pub fn draw(app: &App, buffer: &mut ratatui::buffer::Buffer)` in `ui/mod.rs` is the only entry point; every screen draws through it. `app.rs`, `store/`, `db/` and `runtime.rs` never import `ui`. Trusted tests build the buffer themselves (`Buffer::empty(Rect::new(0, 0, 100, 30))`), so no `Frame`, no `TestBackend`, and no terminal is needed.

## Trusted verification material (planner-shipped, protected)

- **D-070 `tests/support/mod.rs`.** Imported by every trusted test as `mod support;`. Provides the 100x30 render helpers `render_app(&App) -> String` (`ui::draw` into a fresh buffer, then `pgtui::render::buffer_to_text`), `render_app_svg(&App) -> String`, `render_app_png_dims(&App) -> (u32, u32)`, key constructors `key(char)`, `key_code(KeyCode)`, `ctrl(char)`, `enter()`, `tab()`, `back_tab()`, `backspace()`, `esc()`, `down()`, `up()`, `left()`, `right()`, `temp_store() -> (TempDir, ConnectionStore)`, `new_connection(name)`, `saved_connection(name)`, `unwritable_db_path()`; from TASK-004 also `fake_data` and `pg_container() -> (ContainerAsync<Postgres>, ConnParams)` starting `postgres:16-alpine` with `fixtures/seed.sql` applied via `with_init_sql`. The helper set grows per task; earlier tasks' tests keep compiling unchanged.

- **D-071 Behavioural verification, not golden snapshots.** Trusted tests assert behaviour only: substring presence, line ordering and counts on rendered text, store roundtrips, exit codes, and PNG dimensions read from the IHDR chunk. There are no insta snapshots and no golden bytes: any correct implementation passes, and the material stays valid when formatting or styling changes that do not alter behaviour. Every trusted test fails (compile error or assertion) while the feature it covers is absent. No `*.snap` files exist anywhere in the repository.

- **D-072 Seed fixture** `tests/fixtures/seed.sql` (mirrored cell-for-cell as text by `tests/support/fake_data.rs`; the empty string in a fixture row means SQL NULL): schema `public` — `customers(id int PK, name text, balance numeric(10,2), signup_date date, note text NULL)` 5 rows (two NULL notes, balances 10.50, 250.00, -3.25, 99.99, 250.00 to exercise ties and negatives); `orders(id int PK, customer_id int, total numeric(10,2), status text)` 7 rows; `empty_table(id int PK)` 0 rows; schema `audit` — `events(id bigint PK, kind text, at timestamptz)` 3 rows. Sidebar order under D-025: `audit.events`, `public.customers`, `public.empty_table`, `public.orders`. Larger data (`generate_series`) is created inside the test that needs it.

## Gallery (TASK-007)

- **D-080 `gallery` binary contract.**
  - CLI (clap): `gallery [--out <dir>]`, default `docs/screens`; the directory is created with `create_dir_all`.
  - The ten screen names, fixed: `screen__connection_list_empty`, `screen__connection_list_two`, `screen__create_form_blank`, `screen__create_form_filled`, `screen__browser_sidebar_empty_body`, `screen__preview_unsorted`, `screen__preview_sorted_asc`, `screen__preview_sorted_desc`, `screen__custom_sql_empty`, `screen__custom_sql_results`.
  - For each name, build the `App` state exactly as the corresponding `screen_*_test.rs` builds it (same fake connections, same `fake_data` result sets, same cursor/sort/focus), render at 100x30 with `pgtui::render`, and write `<dir>/<name>.svg` (the `buffer_to_svg` string) and `<dir>/<name>.png` (the `svg_to_png` bytes).
  - Output is deterministic: no timestamps, no random values, no environment-dependent paths in the SVG.
  - Exit codes: `0` success; `2` bad arguments or unwritable output directory (`error: <msg>` on stderr); `1` render failure.
  - The bin contains no rendering, font, or SVG code of its own and must not `#[path]`-include anything under `tests/`; it re-implements the tiny state builders locally.
  - `docs/screens/` holds one committed run of `gallery` with the default `--out`; the repository `README.md` gets a `## Screens` section listing the ten names with their `docs/screens/<name>.png` paths. This is the only task that creates `README.md` (D-005).
