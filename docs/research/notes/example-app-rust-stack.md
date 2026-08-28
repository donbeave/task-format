# Example app (fixture) — Rust stack research

Date: 2026-08-28. Versions read from crates.io API + upstream `Cargo.toml`/source on GitHub main; everything not marked UNVERIFIED was confirmed against those sources today.

App under test: terminal PostgreSQL browser. Screens: (1) connection list, (2) create connection, (3) connected view (left sidebar = tables, main body), (4) table preview (`SELECT * FROM <t> LIMIT 500`, per-column asc/desc sort), (5) custom SQL + result grid, (6) disconnect/exit. Connections persisted in a local SQLite file via Turso (Rust rewrite of SQLite).

## 1. Toolchain

| Item | Value | Source |
| --- | --- | --- |
| Rust stable | 1.98.0 (2026-08-20); 1.97.0 (07-09), 1.96.0 (05-28) | rust-lang/rust RELEASES.md |
| Edition 2024 | stabilized in Rust 1.85.0 (2025-02-20); needs `edition = "2024"` + `rust-version >= "1.85"` | known; RELEASES.md |
| Project MSRV | **1.94** (forced by sqlx 0.9.0 `rust-version = 1.94.0`; next highest: ratatui 1.88, etcetera 1.87, clap 1.85, tokio-postgres 1.85) | crates.io metadata |
| cargo-nextest | 0.9.143 (2026-08-04); its own build MSRV 1.91 — irrelevant when installed as binary | crates.io, nextest workspace Cargo.toml |
| just | 1.58.0 (2026-08-03), MSRV 1.89, edition 2024 | crates.io, casey/just Cargo.toml |

Edition 2024 gotchas that matter here: `unsafe extern`, RPIT lifetime capture (`impl Trait` in return position captures all in-scope lifetimes — add `+ use<>` when returning borrows from `&self`), `gen` reserved, `if let` temporaries drop earlier, `tail expression` temporaries drop earlier (fixes some borrow errors, breaks a few), `#[no_mangle]` needs `unsafe(...)`. None of the listed crates need edition-2024-specific features; all compile under it.

## 2. Recommended `Cargo.toml` dependency block (exact versions, 2026-08-28)

```toml
[package]
name = "pgtui"                # working name
version = "0.1.0"
edition = "2024"
rust-version = "1.94"

[dependencies]
# TUI
ratatui   = "0.30.2"                        # MSRV 1.88, edition 2024; default features = crossterm 0.29 backend
crossterm = { version = "0.29.0", features = ["event-stream"] }  # EventStream for tokio::select!
tui-input = { version = "0.15.4", features = ["crossterm"] }     # single-line text fields; depends on ratatui ^0.30.2
unicode-width = "0.2.2"

# async
tokio      = { version = "1.53.1", features = ["rt-multi-thread", "macros", "sync", "time", "fs"] }
tokio-util = "0.7.19"                       # CancellationToken
futures    = "0.3.34"                       # StreamExt for crossterm EventStream

# storage
turso = "0.7.2"                             # local SQLite-compatible DB (tursodatabase/turso, Rust rewrite). NOT libsql.
sqlx  = { version = "0.9.0", default-features = false,
          features = ["runtime-tokio", "tls-rustls-ring-webpki", "postgres"] }

# CLI / errors / config
clap       = { version = "4.6.6", features = ["derive"] }
color-eyre = "0.6.5"
thiserror  = "2.0.20"
directories = "6.0.0"

# misc
serde      = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
tracing    = "0.1.44"
tracing-subscriber = { version = "0.3.23", features = ["env-filter"] }

[dev-dependencies]
# ratatui TestBackend is built in; no extra crate needed for snapshot-less tests
```

Other current versions checked but not selected: `tokio-postgres` 0.7.18 (MSRV 1.85, edition 2024), `anyhow` 1.0.104, `etcetera` 0.11.0 (MSRV 1.87), `tui-textarea` 0.7.0 (**stale**, pins ratatui ^0.29 — incompatible with 0.30), `chrono` 0.4.45, `uuid` 1.26.0, `rust_decimal` 1.42.1, `libsql` 0.9.30 (the fork; not used).

Sub-crates of ratatui 0.30.2 for reference: `ratatui-core` 0.1.2, `ratatui-widgets` 0.3.2, `ratatui-crossterm` 0.1.2 (supports crossterm 0.28 and 0.29 via features `crossterm_0_28` / `crossterm_0_29`; default is 0.29).

## 3. Per-crate rationale + key API

### 3.1 Turso (`turso` 0.7.2) — connection store

Identity: crate `turso` on crates.io, source `tursodatabase/turso/bindings/rust`, MIT. Depends on `turso_core` 0.7.2 + `turso_sdk_kit` + `turso_sync_sdk_kit`. Latest stable 0.7.2 (2026-07-30); 0.8.0-pre.7 (2026-08-21) is a pre-release — pin `0.7.2`.

Status (README, 2026-08): "production-ready with caveats": powers Turso Cloud, Kin AI, Spice.ai; **not 1.0**; recommends independent backups until 1.0; targets SQLite 3.50.4 compatibility, "any undocumented deviation is a bug". Experimental (opt-in via `Builder::experimental_*`): encryption, generated columns, WITHOUT ROWID, materialized views, multiprocess WAL, custom types, attach, vacuum, Postgres-compat. Core DDL/DML, PK AUTOINCREMENT, UNIQUE, DEFAULT, transactions/SAVEPOINT, ALTER TABLE, indexes, datetime functions, WAL, common PRAGMAs: supported (COMPAT.md). Verdict for the fixture: fit — a single-process, single-table config store is squarely in the supported set.

Features (0.7.2): `default = ["mimalloc", "fts"]`, `sync` (Turso Cloud push/pull — not needed), `pure-rust-crypto`, `stacker`, `io_memory_yield`, `test_helper`. Edition 2021; no `rust-version` declared; repo `rust-toolchain.toml` pins 1.88 (UNVERIFIED that it builds on anything older; irrelevant with MSRV 1.94).

API (confirmed from `bindings/rust/src/{lib,connection,rows,value}.rs` + docs.rs 0.7.2):

```rust
use turso::{Builder, Value};

// open (file path or ":memory:"); Database: Clone; Connection: Send + Sync + Clone
let db   = Builder::new_local(path.to_str().unwrap()).build().await?;
let conn = db.connect()?;

conn.execute_batch(
    "CREATE TABLE IF NOT EXISTS connections (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL UNIQUE,
        host TEXT NOT NULL, port INTEGER NOT NULL DEFAULT 5432,
        dbname TEXT NOT NULL, username TEXT NOT NULL, password TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT (datetime('now')));",
).await?;

// positional params: tuple/array/Vec of IntoValue; named via turso::named_params!
let n = conn.execute(
    "INSERT INTO connections (name, host, port, dbname, username, password) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    (name.as_str(), host.as_str(), port as i64, dbname.as_str(), user.as_str(), pw.as_str()),
).await?;                                   // -> u64 rows changed
let id = conn.last_insert_rowid();          // sync, i64

// select
let mut rows = conn.query("SELECT id, name, host, port, dbname, username, password FROM connections ORDER BY name", ()).await?;
let names: Vec<String> = rows.column_names();          // also column_count(), columns() -> Vec<Column{name(), decl_type()}>
while let Some(row) = rows.next().await? {             // Rows::next(&mut self) -> Result<Option<Row>>
    let id: i64 = row.get(0)?;                         // Row::get::<T>(idx) -> Result<T>
    let v: Value = row.get_value(1)?;                  // Value::{Null, Integer(i64), Real(f64), Text(String), Blob(Vec<u8>)}
    let name = v.as_text().cloned().unwrap_or_default(); // as_text() -> Option<&String>; as_integer() -> Option<&i64>; as_real(); as_blob()
}

// prepared: conn.prepare(sql).await? -> Statement (Clone, Send+Sync): .query(params) / .execute(params) / .query_row(params) / .reset() / .n_change()
// delete
conn.execute("DELETE FROM connections WHERE id = ?1", (id,)).await?;
```

Other `Connection` methods: `prepare_cached`, `pragma_query`, `pragma_update`, `is_autocommit`, `busy_timeout(Duration)`, `cacheflush`; `Transaction` via `conn.transaction()` (behaviour enums `TransactionBehavior`, `DropBehavior`). Errors: `turso::Error` (thiserror), `turso::Result<T>`.

Pitfalls:
- Same-connection concurrent writes return `SQLITE_BUSY` (documented deviation). Serialize writes: keep one `Connection` behind the app's DB actor/`Mutex`, never spawn parallel writers.
- Text must be valid UTF-8 (invalid bytes → U+FFFD).
- Do not open the same file concurrently with a C-SQLite process ("mixed SQLite and Turso in multi-process" unsupported). Fixture runs alone; fine.
- `Rows`/`Row` `Send`-ness UNVERIFIED (no `assert_send_sync!` on `Rows` in rows.rs, unlike `Connection`). Mitigation: consume `Rows` to `Vec<T>` inside one async fn on the DB task; do not hold `Rows` across `tokio::spawn` boundaries.
- Pre-1.0: pin exact version; do not use `experimental_*` builder flags in the fixture.
- Default features pull `mimalloc` (C build via `cc`) and `fts`. Whether turso sets `#[global_allocator]` itself is UNVERIFIED; use `turso = { version = "0.7.2", default-features = false }` for a pure-Rust, faster-to-build fixture (the connections store needs neither).

### 3.2 PostgreSQL client: `sqlx` 0.9.0 (chosen) vs `tokio-postgres` 0.7.18

Requirement: run arbitrary user SQL, get column names, render every cell as `String` without knowing types.

| | sqlx 0.9.0 | tokio-postgres 0.7.18 |
| --- | --- | --- |
| Released | 2026-05 (0.9.0; MSRV **1.94**, edition 2021) | 2026-06-12 (MSRV 1.85, edition 2024) |
| Column metadata | `row.columns()` → `&[PgColumn]`; `Column::{name(), ordinal(), type_info()}`; `PgTypeInfo: Display`, `.name()`, `.oid()`, `.kind()` | `row.columns()` → `&[Column]` `.name()`, `.type_()` → `postgres_types::Type` (`Display`, `.oid()`) |
| Generic stringify path | **`sqlx::raw_sql(sql)`** sends the *simple query protocol* (no args ⇒ `queue_simple_query`, result format `PgValueFormat::Text`, verified in `sqlx-postgres/src/connection/executor.rs::run`). Then `row.try_get_raw(i)?` → `PgValueRef` → `.is_null()` / `.as_str()` gives the server's text rendering for **every** type (timestamps, numerics, arrays, json, enums, domains…). No per-type match needed. | `client.simple_query(sql)` → `Vec<SimpleQueryMessage>`; `SimpleQueryMessage::Row(SimpleQueryRow)` with `.columns() -> &[SimpleColumn]` (`.name()`), `.get(i) -> Option<&str>` / `.try_get(i)` — also text for every type. No type info (only names) on `SimpleColumn`; types only via `SimpleQueryMessage::RowDescription`. |
| Extended protocol decode of unknown types | `try_get::<T>` requires `T: Decode + Type`; mismatched type → `ColumnDecode` error. Unusable generically without a type→T match table. | `try_get::<_, T>` same problem (`FromSql`); no raw bytes accessor on `Row`. |
| Pooling | built-in `PgPool` (`PgPoolOptions`) | none (bring `deadpool-postgres`/`bb8`) |
| TLS | feature `tls-rustls-ring-webpki` / `tls-rustls-aws-lc-rs` / `tls-native-tls` / `tls-none` (0.9 removed combined `runtime-tokio-rustls` features) | separate `tokio-postgres-rustls` / `postgres-native-tls` |
| Multi-statement | `raw_sql` executes several `;`-separated statements in one implicit tx | `simple_query` same |
| User SQL safety gate | 0.9: every `query*()`/`raw_sql()` takes `impl SqlSafeStr`; only `&'static str` qualifies — runtime strings must be wrapped `AssertSqlSafe(String)` (explicit "use at your own risk" marker; correct here, the whole point is user SQL) | none |

Recommendation: **sqlx 0.9.0** with `runtime-tokio`, `postgres`, `tls-rustls-ring-webpki`, `default-features = false` (drops `any`, `macros`, `json`, `migrate` — no compile-time `query!` macros, so no `DATABASE_URL` needed at build; the fixture must build offline). Reason: built-in pool + connect-string parsing + typed column info + the text-format trick via `raw_sql`, all in one crate; tokio-postgres would additionally need a pool crate and gives no type info on the simple-query path. The cost is MSRV 1.94 (vs 1.85) — acceptable, stable is 1.98.

Dynamic query → grid (verified API):

```rust
use sqlx::{AssertSqlSafe, Column, Row, TypeInfo, ValueRef, postgres::{PgPool, PgPoolOptions}};

pub struct Grid { pub columns: Vec<ColumnMeta>, pub rows: Vec<Vec<String>> }
pub struct ColumnMeta { pub name: String, pub pg_type: String }

pub async fn connect(url: &str) -> sqlx::Result<PgPool> {
    PgPoolOptions::new().max_connections(2).acquire_timeout(Duration::from_secs(5)).connect(url).await
}
// url form: postgres://user:pw@host:5432/db?sslmode=prefer   (built from the Turso-stored record)

pub async fn run_sql(pool: &PgPool, sql: String) -> sqlx::Result<Grid> {
    // raw_sql => simple query protocol => text-format results => as_str() works for any type
    let rows = sqlx::raw_sql(AssertSqlSafe(sql)).fetch_all(pool).await?;
    let Some(first) = rows.first() else { return Ok(Grid { columns: vec![], rows: vec![] }) };
    let columns = first.columns().iter()
        .map(|c| ColumnMeta { name: c.name().to_owned(), pg_type: c.type_info().name().to_owned() })
        .collect();
    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let mut cells = Vec::with_capacity(row.len());
        for i in 0..row.len() {
            let v = row.try_get_raw(i)?;                       // PgValueRef
            cells.push(if v.is_null() { "NULL".into() }
                       else { v.as_str().map_err(|e| sqlx::Error::Decode(e))?.to_owned() });
        }
        out.push(cells);
    }
    Ok(Grid { columns, rows: out })
}

// table list for sidebar
pub async fn list_tables(pool: &PgPool) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar::<_, String>(
        "SELECT format('%I.%I', schemaname, tablename) FROM pg_catalog.pg_tables \
         WHERE schemaname NOT IN ('pg_catalog','information_schema') ORDER BY 1")
        .fetch_all(pool).await
}

// preview with sort (identifiers quoted; never interpolate raw user text into identifiers)
pub fn preview_sql(table: &str, sort: Option<(&str, SortDir)>) -> String {
    let mut s = format!("SELECT * FROM {table} LIMIT 500");   // table already %I-formatted by list_tables
    if let Some((col, dir)) = sort {
        s = format!("SELECT * FROM {table} ORDER BY {} {} LIMIT 500", quote_ident(col), dir.as_sql());
    }
    s
}
```

Notes/pitfalls:
- Empty result set: zero rows ⇒ `rows.first()` is `None` ⇒ column names unavailable from `fetch_all`. If headers are needed for empty results, use `raw_sql(...).fetch_many(pool)` (stream of `Either<PgQueryResult, PgRow>`) — still no `RowDescription` exposure; UNVERIFIED whether sqlx exposes columns for zero-row results. Fallback: `pool.describe(sql)` (`Executor::describe`) returns `Describe<Postgres>` with `columns()` — uses extended protocol PARSE/DESCRIBE, no execution. Cheap and reliable for headers.
- `raw_sql` cannot bind parameters. Fine: preview/custom SQL have none; identifiers are quoted with `quote_ident` (double-quote, escape `"`).
- Sorting server-side (`ORDER BY`) keeps `LIMIT 500` semantics correct (sorting the fetched 500 client-side gives the wrong window). Spec says "per-column asc/desc sort" of the preview — do it server-side by re-issuing the query; sort state lives in `PreviewState`.
- `as_str()` errors on non-UTF-8 `bytea`? Text format renders `bytea` as `\x...` ASCII, so no. Binary-format values only arise via `query()`/`query_as()` with `.bind()`; never call those for user SQL.
- sqlx 0.9 breaking vs 0.8: runtime+TLS feature split; `SqlSafeStr`; `Any` driver needs `install_default_drivers()` (we don't use `any`); `sqlx.toml` optional.
- Connection errors surface as `sqlx::Error::{Io, Tls, Database(Box<dyn DatabaseError>), PoolTimedOut}`; map `Database(e)` → `e.message()` for the status bar.

### 3.3 TUI: `ratatui` 0.30.2 + `crossterm` 0.29.0

ratatui 0.30.x (0.30.0 2025-12-26; 0.30.1 2026-06-05 bumped MSRV to 1.88; 0.30.2 2026-06-19), edition 2024, workspace split into `ratatui-core`/`ratatui-widgets`/`ratatui-crossterm`; `no_std`-capable core. Default features: `all-widgets, crossterm (=0.29), layout-cache, macros, underline-color`. `ratatui::{init, try_init, restore, run}` install a panic hook that restores the terminal (`ratatui/src/init.rs`: "All initialization functions install a panic hook … call after any other panic hooks") — so call `color_eyre::install()` **before** `ratatui::init()`.

Bootstrap + async event loop (crossterm `event-stream` feature):

```rust
// main.rs
#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;                       // first
    let cli = Cli::parse();
    let terminal = ratatui::init();               // DefaultTerminal = Terminal<CrosstermBackend<Stdout>>; panic hook installed here
    let result = App::new(cli).await?.run(terminal).await;
    ratatui::restore();
    result
}

// event.rs — crossterm EventStream + tokio::select!
use crossterm::event::{Event as CtEvent, EventStream, KeyEvent};
use futures::StreamExt;
pub enum Event { Key(KeyEvent), Resize(u16, u16), Tick, Db(DbResult) }

let mut reader = EventStream::new();
let mut tick = tokio::time::interval(Duration::from_millis(250));
loop {
    tokio::select! {
        _ = cancel.cancelled() => break,
        Some(Ok(ev)) = reader.next() => match ev {
            CtEvent::Key(k) if k.is_press() => tx.send(Event::Key(k))?,   // 0.29: filter Release/Repeat on Windows
            CtEvent::Resize(w, h) => tx.send(Event::Resize(w, h))?,
            _ => {}
        },
        Some(r) = db_rx.recv() => tx.send(Event::Db(r))?,
        _ = tick.tick() => tx.send(Event::Tick)?,
    }
}
```

Layout (sidebar + body + status line):

```rust
use ratatui::layout::{Constraint, Layout, Rect};
fn view_connected(frame: &mut Frame, s: &ConnectedState) {
    let [main, status] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());
    let [sidebar, body] = Layout::horizontal([Constraint::Length(30), Constraint::Fill(1)]).areas(main);
    let tables = List::new(s.tables.iter().map(String::as_str))
        .block(Block::bordered().title("Tables"))
        .highlight_style(Style::new().reversed());
    frame.render_stateful_widget(tables, sidebar, &mut s.sidebar.list_state.clone()); // keep ListState in app state
    render_grid(frame, body, &s.grid, &mut s.grid_state);
    frame.render_widget(Line::from(s.status.as_str()), status);
}
```

Table widget (no built-in sorting — sort indicator is just header text):

```rust
use ratatui::widgets::{Block, Cell, Row, Table, TableState, HighlightSpacing};
pub struct GridState { pub table: TableState, pub sort: Option<(usize, SortDir)>, pub col_offset: usize }

fn render_grid(frame: &mut Frame, area: Rect, grid: &Grid, st: &mut GridState) {
    let header = Row::new(grid.columns.iter().enumerate().map(|(i, c)| {
        let mark = match st.sort { Some((j, SortDir::Asc)) if j == i => " ▲", Some((j, SortDir::Desc)) if j == i => " ▼", _ => "" };
        Cell::from(format!("{}{mark}", c.name))
    })).style(Style::new().bold()).bottom_margin(0);
    let rows = grid.rows.iter().map(|r| Row::new(r.iter().map(|c| Cell::from(c.as_str()))));
    let widths = grid.column_widths(area.width);   // Vec<Constraint::Length(n)>, n = min(max(display width of header/cells), 40)
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::bordered().title(format!("{} rows", grid.rows.len())))
        .row_highlight_style(Style::new().reversed())
        .column_highlight_style(Style::new().underlined())
        .highlight_symbol("> ")
        .highlight_spacing(HighlightSpacing::Always)
        .column_spacing(1);
    frame.render_stateful_widget(table, area, &mut st.table);
}
// TableState: select(Option<usize>), select_next/previous/first/last, select_column(Option<usize>), scroll_down_by(u16), offset()
```

Pitfalls (ratatui):
- `TableState`/`ListState` live in app state, never constructed in the view (else selection resets each frame).
- Table has no horizontal scrolling of columns; with many columns, slice `columns[col_offset..]` before building `Row`s and keep `col_offset` in state (h/l keys). Widths via `unicode-width` (`UnicodeWidthStr::width`), cap per column, truncate cell text (`Cell` does not wrap).
- 500 rows × N cols: rebuilding `Row`s each frame is fine (ratatui renders only the visible window), but keep the `Vec<Vec<String>>` immutable and sorted server-side; don't clone the grid per frame.
- `Style` is `const`-constructible in 0.30 (`Style::new().bold()` in `const`); `Style` no longer implements `Styled`.
- crossterm 0.29 on Windows emits key Release/Repeat; filter with `KeyEvent::is_press()` (or `KeyEventKind::Press`). Kitty protocol not needed.
- `tui-textarea` 0.7.0 pins ratatui ^0.29 → conflicts; use `tui-input` 0.15.4 (ratatui ^0.30.2, crossterm 0.29) for the create-connection form fields and the SQL input. For a multi-line SQL editor either `tui-input` (single line, adequate for fixture) or `edtui` 0.11.7 (UNVERIFIED ratatui 0.30 compat).
- Testing: `ratatui::backend::TestBackend::new(w, h)` + `Terminal::new(backend)`, assert on `terminal.backend().buffer()` (`Buffer::with_lines([...])`) — deterministic, no snapshot crate needed.

### 3.4 `clap` 4.6.6 (derive)

MSRV 1.85, edition 2024. `features = ["derive"]` (default features give color/help/suggestions).

```rust
#[derive(clap::Parser, Debug)]
#[command(name = "pgtui", version, about)]
pub struct Cli {
    /// Path to the connections DB (default: <data_dir>/pgtui/connections.db)
    #[arg(long, env = "PGTUI_DB")] pub db: Option<PathBuf>,
    /// Open this saved connection immediately
    #[arg(long)] pub connect: Option<String>,
    /// Tick interval in ms
    #[arg(long, default_value_t = 250)] pub tick_ms: u64,
}
```

### 3.5 Errors: `color-eyre` 0.6.5 (app/bin) + `thiserror` 2.0.20 (lib)

Convention: library modules (`db`, `store`) define typed `thiserror` enums; the binary/app layer uses `color_eyre::Result` (`eyre::Report`), `.wrap_err("…")` via `color_eyre::eyre::WrapErr`. `anyhow` 1.0.104 is equivalent for the bin layer but `color-eyre` adds the panic/span report and is what ratatui's docs recommend. `thiserror` 2.x: `#[error(transparent)] #[from]` unchanged; supports `#[error(fmt = ...)]`.

```rust
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("turso: {0}")] Turso(#[from] turso::Error),
    #[error("connection {0:?} not found")] NotFound(String),
}
#[derive(Debug, thiserror::Error)]
pub enum PgError {
    #[error("{0}")] Sqlx(#[from] sqlx::Error),
    #[error("invalid connection settings: {0}")] Settings(String),
}
```

### 3.6 Config dir: `directories` 6.0.0 (chosen) vs `etcetera` 0.11.0

`directories::ProjectDirs::from("dev", "task-format", "pgtui")` → `data_dir()`: Linux `$XDG_DATA_HOME/pgtui` (`~/.local/share/pgtui`), macOS `~/Library/Application Support/dev.task-format.pgtui`. Use `data_dir()/connections.db`, honour `--db`/`PGTUI_DB` override (needed so the test harness can point the fixture at a temp file). `etcetera` 0.11 (MSRV 1.87, edition 2024) is the lighter alternative with a strategy switch (XDG-on-macOS); pick `directories` because zero API decisions; either is fine.

## 4. Architecture sketch

Single crate, lib + bin (workspace only if a second crate appears, e.g. a `pgtui-fixtures` helper):

```
Cargo.toml            # [lib] name="pgtui", [[bin]] name="pgtui" path="src/main.rs"
justfile
.config/nextest.toml
src/
  main.rs             # clap parse, color_eyre::install, ratatui::init/restore, App::run
  lib.rs              # pub mod app; pub mod db; pub mod store; pub mod ui; pub mod event; pub mod cli;
  cli.rs              # Cli (clap derive)
  event.rs            # Event enum, EventStream task, tokio::select!, CancellationToken
  app/
    mod.rs            # App { screen: Screen, store: Store, pg: Option<PgSession>, tx/rx } ; run loop
    screen.rs         # Screen enum (state machine) + per-screen state structs
    update.rs         # fn update(app, Event) -> Vec<Cmd>   (pure; no IO)
    cmd.rs            # Cmd enum (async side effects) + executor -> sends Event::Db back
    keymap.rs         # Key -> Message per screen
  store/              # Turso: ConnectionRecord, Store::{open, list, insert, delete}
    mod.rs
    schema.rs
  db/                 # Postgres: PgSession::{connect, list_tables, preview, run_sql, close}, Grid, ColumnMeta, quote_ident
    mod.rs
    grid.rs
  ui/                 # pure view fns: fn view(frame, &App); one file per screen + widgets/grid.rs, widgets/form.rs
    mod.rs
    connections.rs create.rs connected.rs preview.rs sql.rs grid.rs form.rs status.rs
tests/
  store.rs            # Turso round-trip on tempfile
  grid.rs             # Grid rendering via TestBackend
  pg.rs               # #[ignore] unless PGTUI_TEST_PG_URL set: run_sql against real PG
```

State machine (Elm-style: `Screen` is the model variant, `Event`/`Message` drives `update`, `Cmd` runs IO on tokio and reports back as `Event::Db`):

```rust
pub enum Screen {
    Connections(ConnectionsState),        // list: Vec<ConnectionRecord>, ListState; keys: j/k, Enter=connect, n=new, d=delete, q=exit
    CreateConnection(FormState),          // fields: name host port dbname username password; Tab/Shift-Tab, Enter=save, Esc=cancel
    Connecting { record: ConnectionRecord },                        // spinner; Cmd::Connect in flight
    Connected(ConnectedState),            // sidebar tables + body: Body::{Empty, Preview(PreviewState), Sql(SqlState)}
    Exiting,
}
pub struct ConnectedState { pub conn_name: String, pub tables: Vec<String>, pub sidebar: ListState, pub focus: Focus /*Sidebar|Body*/, pub body: Body, pub status: String }
pub struct PreviewState { pub table: String, pub grid: Grid, pub grid_state: GridState, pub sort: Option<(usize, SortDir)>, pub loading: bool }
pub struct SqlState { pub input: tui_input::Input, pub grid: Option<Grid>, pub grid_state: GridState, pub error: Option<String> }

pub enum Cmd { LoadConnections, SaveConnection(NewConnection), DeleteConnection(i64),
               Connect(ConnectionRecord), ListTables, Preview { table: String, sort: Option<(String, SortDir)> },
               RunSql(String), Disconnect }
pub enum DbResult { Connections(Vec<ConnectionRecord>), Saved(i64), Deleted, Connected(PgPool), Tables(Vec<String>),
                    Grid(Grid), Error(String), Disconnected }
```

Transitions (the 6 screens):

| From | Key/Event | To |
| --- | --- | --- |
| Connections | `n` | CreateConnection |
| Connections | Enter | Connecting → (`DbResult::Connected`) Connected{body: Empty} / (`Error`) Connections + status |
| Connections | `q` / Ctrl-C | Exiting (break loop, drop pool, restore terminal) |
| CreateConnection | Enter (valid) | Cmd::SaveConnection → Connections (reloaded) |
| CreateConnection | Esc | Connections |
| Connected(sidebar) | Enter on table | body = Preview (Cmd::Preview{sort: None}) |
| Connected(Preview) | `s`/`S` or `←/→`+`s` on column | Cmd::Preview{sort: Some((col, toggled dir))} → same screen, grid replaced |
| Connected(any) | `:` or F5 | body = Sql (input focused); Enter = Cmd::RunSql; result → grid or error |
| Connected(any) | `Esc` from Sql/Preview | body = previous/Empty |
| Connected(any) | `D` (disconnect) | Cmd::Disconnect → Connections |
| Connected(any) | `q` | Exiting |

Runtime shape: `App::run` owns `Terminal`; loop = `terminal.draw(|f| ui::view(f, &mut app))` → `rx.recv().await` → `update()` → for each `Cmd`, `tokio::spawn` with a clone of `PgPool`/`Store` handle, send `Event::Db(DbResult)` back through the same `mpsc::UnboundedSender`. Keeps `update` synchronous and testable. Turso `Connection` is `Send + Sync + Clone`, so `Store` can be cloned into spawned tasks; keep writes sequential (one `Cmd` in flight per store — `SaveConnection`/`DeleteConnection` set a `busy` flag).

Testability hooks the fixture needs (so task variants can be verified by `verify.sh`): `--db <tempfile>`; `PGTUI_TEST_PG_URL` for integration tests; `Grid` and `update()` pure and unit-testable; `ui::view` rendered through `TestBackend` for golden-buffer assertions; `cargo nextest run` with `.config/nextest.toml` (`[profile.default] fail-fast = false`, `slow-timeout = { period = "30s" }`); `justfile` recipes `build`, `test` (`cargo nextest run`), `lint` (`cargo clippy --all-targets -- -D warnings`), `fmt-check`, `run`.

## 5. Open items / UNVERIFIED

- Turso `Rows`/`Row` `Send` bound (mitigated by design above).
- Turso 0.7.2 behaviour when the DB file's parent dir is missing (create dir with `std::fs::create_dir_all` first).
- sqlx: obtaining column headers for a zero-row `raw_sql` result (use `Executor::describe` fallback).
- `edtui` 0.11.7 ratatui-0.30 compatibility (only needed if a multi-line SQL editor is wanted; `tui-input` 0.15.4 is verified against ratatui ^0.30.2).
- ratatui 0.30 `Rect::layout()` ergonomic API exists per changelog; snippets above use the long-stable `Layout::{horizontal,vertical}(..).areas(rect)`.
- Windows terminal behaviour (fixture runs on macOS/Linux containers only).
