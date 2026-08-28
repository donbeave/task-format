# TASK-105 — fixed decisions (verbatim)

Planner-owned. Copied from the decomposition plan; numbering is global across tasks. Read-only. Anything not covered here that changes public behavior is `NEEDS_REPLAN`.

## D-010 Screen enum and App fields

```rust
pub enum Screen { ConnectionList, CreateConnection, Browser, CustomSql }
```

`App` fields (all pub for tests): `screen: Screen`, `connections: Vec<SavedConnection>`, `list_cursor: usize`, `form: CreateForm`, `session: Option<SessionView>` (`SessionView { name: String, tables: Vec<TableRef>, sidebar_cursor: usize, focus: Focus }`, `enum Focus { Sidebar, Grid }`), `grid: Option<Grid>`, `sql_input: String`, `sql_grid: Option<Grid>`, `status: Status` (`enum Status { Help, Info(String), Error(String) }`), `quit_requested: bool`.

## D-011 Elm-style core

`App::update(&mut self, msg: Msg) -> Vec<Effect>` is pure (no IO, no async). `runtime::execute(&mut Runtime, Effect) -> Option<Msg>` performs IO. Tests drive `App` with `Msg` values directly; snapshots never need a database.

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
    Connect(SavedConnection),
    Query { kind: QueryKind, sql: String },
    Disconnect,
    Quit,
}
```

## D-013 Status line

Bottom row, height 1, full width. `Status::Help` shows the screen's key hint (D-060). `Info`/`Error` replace it until the next key press, then revert to `Help`. Error text is `error: <message>`; message is the first line of the underlying error, truncated to width. No modal dialogs.

## D-025 Queries — custom SQL

Custom SQL is sent verbatim after trimming whitespace and one trailing `;`. If the server returns more than one row description → `DbError::MultiStatement` → `error: one statement at a time`. Non-row results (`CommandComplete`) → `Status::Info("ok: <n> rows affected")`, `sql_grid = None`. Row results are capped client-side at `PREVIEW_LIMIT` (500, `pub const` in `db/mod.rs`); when capped, `Status::Info("showing first 500 rows")`. All statements go through `Client::simple_query` (D-023).

## D-026 ResultSet

`pub struct ResultSet { pub columns: Vec<String>, pub rows: Vec<Vec<Cell>> }`, `pub enum Cell { Null, Text(String) }`. `Cell::Null` renders as `NULL`. `pub struct TableRef { pub schema: String, pub name: String }`, `Display` = `schema.name`.

## D-034 CustomSql keys

Printable chars append to `sql_input`, `Backspace` pops, `Enter` → `Effect::Query { kind: Custom, sql }` (empty input → no-op). `Esc` → `Screen::Browser` (input and result retained). Result grid: `Up`/`Down` move row cursor only; **no sort, no column cursor**. Because letters type into the input, there is no `d`/`q` here; disconnect via `Esc` then `d`, exit via `Ctrl+C` (D-030: `Ctrl+C` is global on every screen).

Entry (D-033): `x` in Browser → `Screen::CustomSql` (keeps session and grid).

## D-041 Error surfacing

Every `Err` reaching `App::update` becomes `Status::Error` (D-013). No error is swallowed; no `unwrap()`/`expect()` on IO results in `src/` outside `main.rs`'s terminal setup.

## D-050 Grid model (frozen from this task on)

```rust
pub struct Grid { pub columns: Vec<String>, rows: Vec<Vec<Cell>>, pub col_cursor: usize, pub row_cursor: usize, pub sort: Option<SortState> }
pub struct SortState { pub column: usize, pub dir: SortDir }
pub enum SortDir { Asc, Desc }
```

`Grid::from(ResultSet)`; `Grid::visible_rows(&self) -> Vec<&Vec<Cell>>`. `sql_grid` reuses this type; its `sort` stays `None` and `col_cursor` stays `0`.

## D-060 Frame (deterministic; snapshots depend on it)

Snapshot terminal 100×30. Root layout: `[Min(0)] + [Length(1)]` → body + status line. Borders: `Borders::ALL`, plain. Titles wrapped in single spaces.

- CustomSql: vertical `[Length(3)] + [Min(0)]`. Input block title ` SQL `, content `> <sql_input>`. Results block title ` Results ` while empty with body line `Type a query and press Enter`; after a result ` Results  <rows> rows `. Help: `Enter run  Up/Down rows  Esc back  Ctrl+C quit`.
- Grid (shared widget): ratatui `Table`; header row = column names; column width = `clamp(max(len(header)+4, max cell len), 4, 24)`; `highlight_symbol` `> ` on cursor row; `Cell::Null` → `NULL`. Custom SQL grid: same widget, **no `[ ]` on any header and no sort markers**.

## D-061 Rendering entry point

`ui::draw(frame: &mut Frame, app: &App)`; tests render with `ratatui::backend::TestBackend::new(100, 30)`.

## D-071 Snapshot policy

Text snapshots via `insta::assert_snapshot!("<name>", render_text(&app))`, SVG via `insta::assert_snapshot!("<name>__svg", render_svg(&app))`. Names in this task: `screen__custom_sql_empty`, `screen__custom_sql_results`. All `.snap` files are shipped by the planner and protected. The gate runs tests with `INSTA_UPDATE=no INSTA_FORCE_PASS=0`; any `*.snap.new` fails the gate.

## D-072 Seed fixture `tests/fixtures/seed.sql`

Schema `public`: `customers(id int PK, name text, balance numeric(10,2), signup_date date, note text NULL)` 5 rows (two NULL notes); `orders(id int PK, customer_id int, total numeric(10,2), status text)` 7 rows; `empty_table(id int PK)` 0 rows; schema `audit`: `events(id bigint PK, kind text, at timestamptz)` 3 rows. `pg_custom_sql_test` creates any larger data it needs (`generate_series`) inside the test.
