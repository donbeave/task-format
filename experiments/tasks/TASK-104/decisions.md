# TASK-104 — fixed decisions (verbatim)

Planner-owned. Copied from the decomposition plan; numbering is global across tasks. Read-only. Anything not covered here that changes public behavior is `NEEDS_REPLAN`.

## D-011 Elm-style core

`App::update(&mut self, msg: Msg) -> Vec<Effect>` is pure (no IO, no async). `runtime::execute(&mut Runtime, Effect) -> Option<Msg>` performs IO. Tests drive `App` with `Msg` values directly; snapshots never need a database. Relevant variants (already in `app.rs` since TASK-101):

```rust
Msg::QueryDone { kind: QueryKind, result: Result<ResultSet, DbError> }
pub enum QueryKind { Preview(TableRef), Custom }
Effect::Query { kind: QueryKind, sql: String }
```

## D-023 Client + protocol

`tokio-postgres` 0.7.18, **simple-query protocol** (`Client::simple_query`) for every statement the app runs. Simple-query returns every cell as text (`Option<&str>`), so preview needs no per-type decoding; NULL is `None`; unknown types cannot fail decoding.

## D-025 Queries (exact SQL; identifiers double-quoted with `"` doubled)

- preview: `SELECT * FROM "<schema>"."<table>" LIMIT 500` — `PREVIEW_LIMIT: usize = 500` const in `db/mod.rs`; no `ORDER BY`. Row order is heap order; the seed fixture is small and freshly inserted, so it is stable in practice. `pg_preview_test::preview_matches_fake` compares real rows to fake rows as multisets (sorted by primary key inside the test).
- `pg_preview_test::limit_applied_on_600_row_table` creates its own 600-row table inside the test (not in `seed.sql`).
- Custom SQL semantics are TASK-105; `PgSession::query` in this task only needs row results to work and non-row results to not error.

## D-026 ResultSet

`pub struct ResultSet { pub columns: Vec<String>, pub rows: Vec<Vec<Cell>> }`, `pub enum Cell { Null, Text(String) }`. `Cell::Null` renders as `NULL`. `pub struct TableRef { pub schema: String, pub name: String }`, `Display` = `schema.name`.

## D-033 Browser keys

`Tab` toggles `Focus::Sidebar` ⇄ `Focus::Grid` (Grid only if `grid.is_some()`). Sidebar focus: `j`/`k`/`Up`/`Down` move (clamped); `Enter` → `Effect::Query { kind: Preview(t), sql }`. Grid focus: `h`/`Left`, `l`/`Right` move column cursor (clamped); `j`/`k`/`Up`/`Down` move row cursor (clamped); `s` cycles sort on the cursor column (D-052). `x` → `Screen::CustomSql` (TASK-105; no-op here). `d` → `Effect::Disconnect` (TASK-106; no-op here).

## D-041 Error surfacing

Every `Err` reaching `App::update` becomes `Status::Error` (D-013: status text `error: <message>`, first line of the underlying error, truncated to width; replaced by `Help` on the next key press). No error is swallowed; no `unwrap()`/`expect()` on IO results in `src/` outside `main.rs`'s terminal setup.

## D-050 Grid model

```rust
pub struct Grid { pub columns: Vec<String>, rows: Vec<Vec<Cell>>, pub col_cursor: usize, pub row_cursor: usize, pub sort: Option<SortState> }
pub struct SortState { pub column: usize, pub dir: SortDir }
pub enum SortDir { Asc, Desc }
```

`Grid::from(ResultSet)`; `Grid::visible_rows(&self) -> Vec<&Vec<Cell>>` returns the sorted view (original order when `sort == None`); original row order is never mutated.

## D-051 Comparison

Per column: if every non-null cell parses as `f64` → numeric compare; else byte-wise `str` compare. Stable sort (`sort_by`); ties keep original order.

## D-052 Null placement + cycling

Asc: NULLs last. Desc: NULLs first (PostgreSQL semantics). `s` on cursor column: `None → Asc → Desc → None`; `s` on a different column starts at `Asc` and replaces the previous sort. Sort state resets when a new preview loads.

## D-053 Sort is client-side

Over the fetched ≤500 rows; no re-query.

## D-060 Frame (deterministic; snapshots depend on it)

Snapshot terminal 100×30. Root layout: `[Min(0)] + [Length(1)]` → body + status line. Borders: `Borders::ALL`, plain. Titles wrapped in single spaces.

- Browser: horizontal `[Length(30)] + [Min(0)]`. Sidebar block title ` Tables (<n>) `, rows `schema.name`, `highlight_symbol` `> `. Main block title ` <connection name> ` while `grid.is_none()` (body empty), else ` <schema.table>  <rows> rows  limit 500 `. The focused pane's title gets suffix `*` inside the brackets (e.g. ` Tables (3)* `). Help: `Tab focus  j/k move  Enter preview  h/l col  s sort  x sql  d disconnect`.
- Grid (shared widget): ratatui `Table`; header row = column names; cursor column header wrapped as `[name]`; sorted column header suffixed ` ^` (asc) or ` v` (desc), e.g. `[balance ^]`; column width = `clamp(max(len(header)+4, max cell len), 4, 24)`; `highlight_symbol` `> ` on cursor row; `Cell::Null` → `NULL`.
- Status line (D-013): bottom row, height 1, full width; `Status::Help` shows the screen's key hint; `Info`/`Error` replace it until the next key press.

## D-061 Rendering entry point

`ui::draw(frame: &mut Frame, app: &App)`; tests render with `ratatui::backend::TestBackend::new(100, 30)`.

## D-070 `tests/support/mod.rs` (planner-shipped, protected)

Provides `render_text(&App) -> String`, `render_svg(&App) -> String`, `svg_to_png(&str) -> Vec<u8>`, `key(char)`, `key_code(KeyCode)`, `ctrl(char)`, `temp_store()`, `pg_container() -> (Container, ConnParams)` starting `postgres:16-alpine` and applying `fixtures/seed.sql`, and `fake_data::{tables(), preview(table) -> ResultSet}` mirroring `seed.sql` exactly.

## D-071 Snapshot policy

Text snapshots via `insta::assert_snapshot!("<name>", render_text(&app))`, SVG via `insta::assert_snapshot!("<name>__svg", render_svg(&app))`. Names in this task: `screen__preview_unsorted`, `screen__preview_sorted_asc`, `screen__preview_sorted_desc`. All `.snap` files are shipped by the planner and protected. The gate runs tests with `INSTA_UPDATE=no INSTA_FORCE_PASS=0`; any `*.snap.new` fails the gate.

## D-072 Seed fixture `tests/fixtures/seed.sql`

Schema `public`: `customers(id int PK, name text, balance numeric(10,2), signup_date date, note text NULL)` 5 rows (two NULL notes, balances 10.50, 250.00, -3.25, 99.99, 250.00 to exercise ties and negatives); `orders(id int PK, customer_id int, total numeric(10,2), status text)` 7 rows; `empty_table(id int PK)` 0 rows; schema `audit`: `events(id bigint PK, kind text, at timestamptz)` 3 rows. Sidebar order: `audit.events, public.customers, public.empty_table, public.orders`.
