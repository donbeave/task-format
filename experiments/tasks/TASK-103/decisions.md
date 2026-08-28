# TASK-103 — fixed decisions (verbatim)

Planner-owned. Numbering is global across the pgtui task series. Implement; do not reopen. D-001..D-005 (workspace, module layout, test placement, lint, justfile), D-020..D-022 (store, frozen) and D-030..D-032 (keys of earlier screens) are unchanged and remain in force.

## App state machine

- **D-010 Screen enum** (exact): `pub enum Screen { ConnectionList, CreateConnection, Browser, CustomSql }`. `App` fields (all pub for tests): `screen`, `connections: Vec<SavedConnection>`, `list_cursor: usize`, `form: CreateForm`, `session: Option<SessionView>` (`pub struct SessionView { pub name: String, pub tables: Vec<TableRef>, pub sidebar_cursor: usize, pub focus: Focus }`, `pub enum Focus { Sidebar, Grid }`), `grid: Option<Grid>`, `sql_input: String`, `sql_grid: Option<Grid>`, `status: Status` (`enum Status { Help, Info(String), Error(String) }`), `quit_requested: bool`.
- **D-011 Elm-style core.** `App::update(&mut self, msg: Msg) -> Vec<Effect>` is pure (no IO, no async). `runtime::execute(&mut Runtime, Effect) -> Option<Msg>` performs IO. `Runtime` owns `ConnectionStore` and `session: Option<PgSession>`. Relevant variants: `Msg::Connected(Result<Vec<TableRef>, DbError>)`, `Effect::Connect(SavedConnection)` — the runtime connects AND lists tables, replies `Connected(..)`.
- **D-012 Runtime loop.** `main.rs`: parse CLI → open store → enter raw mode/alternate screen → `App::update(Msg::Connections(store.list()))` → loop { draw; read key (blocking); effects = update(Key); for each effect: execute → feed reply Msg → collect further effects } until `Effect::Quit`. DB calls block the loop (no background tasks) — accepted; the app is single-user. The runtime uses a `tokio` current-thread runtime and `block_on` for the async store/PG calls.
- **D-013 Status line.** Bottom row, height 1, full width. `Status::Help` shows the screen's key hint (D-060). `Info`/`Error` replace it until the next key press, then revert to `Help`. Error text is `error: <message>`; message is the first line of the underlying error, truncated to width. No modal dialogs.

## PostgreSQL

- **D-023 Client + protocol.** `tokio-postgres` 0.7.18, **simple-query protocol** (`Client::simple_query`) for every statement the app runs. Reason: simple-query returns every cell as text (`Option<&str>`), so preview and custom SQL need no per-type decoding, NULL is `None`, and unknown types cannot fail decoding. `SimpleQueryMessage::Row(SimpleQueryRow)`: `.columns()[i].name()`, `.get(i) -> Option<&str>`. No `query`/`query_one`/`query_raw`/`prepare`/`execute` in `db/`.
- **D-024 DSN.** Built from fields, never typed by the user: `tokio_postgres::Config::new().host(&host).port(port).dbname(&dbname).user(&username).password(&password)` (equivalent to `host=<host> port=<port> dbname=<dbname> user=<username> password=<password>`), `NoTls`. No sslmode, no URL parsing. Connect timeout 5 s (`tokio::time::timeout` around `config.connect(NoTls)`); elapsed → `DbError::Timeout`, other errors → `DbError::Connect(<message>)`. The connection future is `tokio::spawn`ed and its `JoinHandle` kept in `PgSession`. Failure is non-fatal: `Msg::Connected(Err)` → status `error: ...`, screen stays `ConnectionList`.
- **D-025 Queries** (exact SQL; identifiers double-quoted with `"` doubled by `quote_ident`):
  - tables: `SELECT table_schema, table_name FROM information_schema.tables WHERE table_type = 'BASE TABLE' AND table_schema NOT IN ('pg_catalog', 'information_schema') ORDER BY table_schema, table_name`
  - preview and custom SQL: later tasks (`PREVIEW_LIMIT: usize = 500` const declared in `db/mod.rs` now).
- **D-026 Types** (`db/mod.rs`, exact):

  ```rust
  pub const PREVIEW_LIMIT: usize = 500;
  pub struct ConnParams { pub host: String, pub port: u16, pub dbname: String, pub user: String, pub password: String }
  impl From<&SavedConnection> for ConnParams { /* field copy */ }
  pub struct TableRef { pub schema: String, pub name: String }   // Display = "schema.name"
  pub struct ResultSet { pub columns: Vec<String>, pub rows: Vec<Vec<Cell>> }
  pub enum Cell { Null, Text(String) }                            // Null renders as NULL
  pub enum DbError { Connect(String), Timeout, Query(String), MultiStatement }   // Display: first line, no prefix
  pub fn quote_ident(s: &str) -> String;                          // "\"" + s.replace('"', "\"\"") + "\""
  ```

  `db/postgres.rs`: `pub struct PgSession { /* private */ }`, `impl PgSession { pub async fn connect(p: &ConnParams) -> Result<Self, DbError>; pub async fn list_tables(&self) -> Result<Vec<TableRef>, DbError>; }`.

## Key bindings

- **D-033 Browser.** `Tab` toggles `Focus::Sidebar` ⇄ `Focus::Grid` (Grid only if `grid.is_some()`; no-op otherwise). Sidebar focus: `j`/`k`/`Up`/`Down` move (clamped, no wrap); `Enter` → `Effect::Query { kind: Preview(t), sql }` (later task; no-op in TASK-103). Grid focus keys: later task. `x` → `Screen::CustomSql` (later task; no-op now). `d` → `Effect::Disconnect` (later task; no-op now). `Ctrl+C` (D-030) → `Effect::Quit` still works from Browser.
- **D-031 ConnectionList (this task activates `Enter`):** `Enter` → `Effect::Connect(selected)`; empty list → no-op.

## Errors

- **D-041 Error surfacing.** Every `Err` reaching `App::update` becomes `Status::Error` (D-013). No error is swallowed; no `unwrap()`/`expect()` on IO results in `src/` outside `main.rs`'s terminal setup.

## Layout (deterministic; snapshots depend on it)

- **D-060 Frame.** Snapshot terminal 100×30. Root layout: `[Min(0)] + [Length(1)]` → body + status line. Borders: `Borders::ALL`, plain. Titles wrapped in single spaces.
  - Browser: horizontal `[Length(30)] + [Min(0)]`. Sidebar block title ` Tables (<n>) `, rows `schema.name`, `highlight_symbol` `> `. Main block title ` <connection name> ` while `grid.is_none()` (body empty), else ` <schema.table>  <rows> rows  limit 500 ` (later task). The focused pane's title gets suffix `*` inside the brackets (e.g. ` Tables (4)* `). Help: `Tab focus  j/k move  Enter preview  h/l col  s sort  x sql  d disconnect`.
- **D-061 Rendering entry point.** `ui::draw(frame: &mut Frame, app: &App)`; tests render with `ratatui::backend::TestBackend::new(100, 30)`.

## Test infrastructure (planner-shipped, protected)

- **D-070 `tests/support/mod.rs`** provides: `render_text(&App)`, `render_svg(&App)`, `svg_to_png`, `key`, `key_code`, `ctrl`, `temp_store()`, `pg_container() -> (ContainerAsync<Postgres>, ConnParams)` starting `postgres:16-alpine` via `testcontainers` 0.28 + `testcontainers-modules` 0.15 with `with_init_sql(seed.sql)`; `fake_data::{tables() -> Vec<TableRef>, preview(&TableRef) -> ResultSet}` mirroring `seed.sql` exactly.
- **D-071 Snapshot policy.** Text via `insta::assert_snapshot!("<name>", render_text(&app))`, SVG via `"<name>__svg"`. Name for this task: `screen__browser_sidebar_empty_body` (connection name `local`, four fake tables, cursor on `public.customers`, Sidebar focused). All `.snap` files are planner-shipped and protected. Gate runs with `INSTA_UPDATE=no INSTA_FORCE_PASS=0`; `*.snap.new` anywhere fails the gate.
- **D-072 Seed fixture** `tests/fixtures/seed.sql`: schema `public`: `customers(id int PK, name text, balance numeric(10,2), signup_date date, note text NULL)` 5 rows (two NULL notes, balances 10.50, 250.00, -3.25, 99.99, 250.00); `orders(id int PK, customer_id int, total numeric(10,2), status text)` 7 rows; `empty_table(id int PK)` 0 rows; schema `audit`: `events(id bigint PK, kind text, at timestamptz)` 3 rows. Sidebar order under D-025: `audit.events, public.customers, public.empty_table, public.orders`.
