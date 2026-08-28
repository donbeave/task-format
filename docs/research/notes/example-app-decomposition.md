# Example app decomposition — `pgtui` (ratatui + Turso + PostgreSQL)

Planning note. Purpose: a real-world fixture to run the task-format experiments (§7 backlog) against. Applies the readiness rules from `reference/task-template/README.md` (one outcome, one canonical gate, 5–20 leaves, no open decisions, verify.sh FAIL on baseline / PASS on reference solution) and D1–D18 from `RESEARCH-FINDINGS.md` §3.

Crate names in this note: `ratatui`, `crossterm`, `turso`, `tokio-postgres` (preferred over `sqlx`; reason in D-020), `testcontainers` (+ `testcontainers-modules` postgres), `insta`, `resvg`/`usvg`/`tiny-skia`, `clap`, `assert_cmd`. Exact versions are being verified by other agents; the fixture `Cargo.toml` pins whatever they report. Nothing below depends on a specific minor version.

---

## 0. Verdict: one task or several?

Several. Six tasks, linear DAG. Reasons, mapped to the readiness rules:

| Rule | Why one task fails it |
| --- | --- |
| One outcome | Seven user-visible behaviours (list, create, connect+sidebar, preview, sort, custom SQL, disconnect/exit) across three subsystems (SQLite store, PG client, TUI). |
| 5–20 leaves | A single task needs ~60 evidence-bearing leaves; capping at 20 would force leaves like "implement the app". |
| One canonical gate | A gate with 10 protected snapshots + PG container tests + store tests on an empty repo gives the executor no incremental signal; everything fails until everything works. |
| verify.sh FAIL on baseline / PASS on reference | Satisfiable, but the reference solution is the whole app — no way to localise a gate defect. |
| ~2,500-token `task.md` | Not reachable with the D-* list below inlined. |

Split follows the suggested one, with one change argued in §4: the **visual (SVG/PNG) pipeline is test infrastructure, so the planner ships it in the TASK-101 fixture as protected support code**; TASK-106 then owns disconnect/exit semantics, exit codes, and the `gallery` binary that renders every screen to `docs/screens/`. Rationale: the render helper has no product behaviour, and putting it into the fixture lets every task from 101 on produce SVG snapshots for its own screens instead of retrofitting them at the end.

---

## 1. Fixed decisions (D-*) — planner-owned, referenced by every task

The executor never chooses architecture. Each `task.md` references the D-* it needs verbatim (copy the text, do not link — a fresh container has no access to this note). Numbering is global across tasks so a D-id means the same thing everywhere.

### Repository / crate layout

- **D-001 Workspace.** Cargo workspace, edition 2024, `resolver = "3"`. Single member `crates/pgtui` (lib + bin `pgtui`). Workspace `Cargo.toml` owns `[workspace.dependencies]` with every dependency pre-declared and pinned at fixture time; `crates/pgtui/Cargo.toml` uses `workspace = true`. **Executors never add, remove, or bump a dependency** (Cargo.toml/Cargo.lock are denied globs in every task).
- **D-002 Module layout** (final; files may be empty stubs at baseline):

  ```text
  crates/pgtui/src/
    main.rs           CLI (clap): --db <path>; terminal setup/teardown; runtime loop; exit codes (D-040)
    lib.rs            pub mod app; keys; store; db; grid; ui; runtime;
    app.rs            App state, Screen enum (D-010), Msg/Effect (D-011), App::update(Msg) -> Vec<Effect>
    keys.rs           KeyEvent -> Action mapping per screen (D-030..D-034)
    store/mod.rs      SavedConnection, NewConnection, StoreError, ConnectionStore (Turso)  (D-020..D-022)
    db/mod.rs         ConnParams, TableRef, ResultSet, Cell, DbError
    db/postgres.rs    PgSession: connect / list_tables / query (simple-query protocol)  (D-023..D-025)
    grid.rs           Grid: columns, rows, cursor, SortState, sorted view  (D-050..D-053)
    runtime.rs        execute(Effect) -> Msg against ConnectionStore + Option<PgSession>
    ui/mod.rs         draw(&mut Frame, &App); layout constants (D-060)
    ui/connection_list.rs  ui/create_form.rs  ui/browser.rs  ui/custom_sql.rs  ui/grid.rs  ui/status.rs
    bin/gallery.rs    TASK-106: render every screen to docs/screens/*.svg|png using fixture data
  crates/pgtui/tests/
    support/mod.rs         planner-shipped, protected (D-070)
    support/fake_data.rs   in-memory ResultSets identical to fixtures/seed.sql
    support/fonts/DejaVuSansMono.ttf
    fixtures/seed.sql      PG seed used by testcontainers tests
    snapshots/*.snap       planner-shipped expected renders (D-071)
    <name>_test.rs         planner-shipped trusted tests, one file per AC group
  ```

- **D-003 Test placement.** Trusted tests (planner) live in `crates/pgtui/tests/*_test.rs` and are protected. Executor-written tests are `#[cfg(test)] mod tests` inside the `src/` module they test; no executor-written file goes under `tests/`.
- **D-004 Toolchain/lint.** `rust-toolchain.toml` pins stable. Gate lint: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`. `#![allow(...)]`, `#[allow(clippy::...)]`, `todo!()`, `unimplemented!()`, `dbg!()` forbidden in `src/`.
- **D-005 `justfile`** recipes (fixture, read-only for executors): `test`, `test-unit` (`--lib`), `test-pg` (`--test 'pg_*'`), `lint`, `snap` (`cargo insta test --check`), `gallery` (TASK-106). Executors call `cargo` directly; `just` is a convenience for the operator.

### App state machine

- **D-010 Screen enum** (exact):

  ```rust
  pub enum Screen { ConnectionList, CreateConnection, Browser, CustomSql }
  ```

  `App` fields (all pub for tests): `screen: Screen`, `connections: Vec<SavedConnection>`, `list_cursor: usize`, `form: CreateForm`, `session: Option<SessionView>` (`SessionView { name: String, tables: Vec<TableRef>, sidebar_cursor: usize, focus: Focus }`, `enum Focus { Sidebar, Grid }`), `grid: Option<Grid>`, `sql_input: String`, `sql_grid: Option<Grid>`, `status: Status` (`enum Status { Help, Info(String), Error(String) }`), `quit_requested: bool`.
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

### Storage (Turso)

- **D-020 Store engine.** `turso` (local SQLite file). Path precedence: `--db <path>` > `$PGTUI_DB` > `$XDG_DATA_HOME/pgtui/connections.db` (fallback `~/.local/share/pgtui/connections.db`). Parent dir created with `create_dir_all`; failure → exit 2.
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

  Password stored plaintext. Keyring integration is explicitly out of scope for all six tasks (deferred; note in each task's out-of-scope).
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

### PostgreSQL

- **D-023 Client + protocol.** `tokio-postgres`, **simple-query protocol** (`Client::simple_query`) for every statement the app runs. Reason: simple-query returns every cell as text (`Option<&str>`), so preview and custom SQL need no per-type decoding, NULL is `None`, and unknown types cannot fail decoding. If the version-verification agents overturn `tokio-postgres`, the replacement must offer an equivalent all-text execution path; that is a re-plan of D-023, not executor discretion.
- **D-024 DSN.** Built from fields, never typed by the user: `host=<host> port=<port> dbname=<dbname> user=<username> password=<password>` (tokio-postgres key/value config, `NoTls`). No sslmode, no URL parsing. Connect timeout 5 s (`tokio::time::timeout` around connect). Failure is non-fatal: `Msg::Connected(Err)` → status `error: ...`, screen stays `ConnectionList`.
- **D-025 Queries** (exact SQL; identifiers double-quoted with `"` doubled):
  - tables: `SELECT table_schema, table_name FROM information_schema.tables WHERE table_type = 'BASE TABLE' AND table_schema NOT IN ('pg_catalog', 'information_schema') ORDER BY table_schema, table_name`
  - preview: `SELECT * FROM "<schema>"."<table>" LIMIT 500` — `PREVIEW_LIMIT: usize = 500` const in `db/mod.rs`; no `ORDER BY` (spec). Row order is heap order; the seed fixture is small and freshly inserted, so it is stable in practice (risk accepted, see §5).
  - custom SQL: sent verbatim after trimming whitespace and one trailing `;`. If the server returns more than one row description → `DbError::MultiStatement` → `error: one statement at a time`. Non-row results (`CommandComplete`) → `Status::Info("ok: <n> rows affected")`, `sql_grid = None`. Row results are capped client-side at `PREVIEW_LIMIT`; when capped, `Status::Info("showing first 500 rows")`.
- **D-026 ResultSet.** `pub struct ResultSet { pub columns: Vec<String>, pub rows: Vec<Vec<Cell>> }`, `pub enum Cell { Null, Text(String) }`. `Cell::Null` renders as `NULL`. `pub struct TableRef { pub schema: String, pub name: String }`, `Display` = `schema.name`.

### Key bindings (single source: `keys.rs`)

- **D-030 Global.** `Ctrl+C` → `Effect::Disconnect` (if connected) then `Effect::Quit`, from every screen. No other global key.
- **D-031 ConnectionList.** `j`/`Down` cursor +1, `k`/`Up` cursor −1 (clamped, no wrap); `Enter` → `Effect::Connect(selected)`; `n` → `Screen::CreateConnection` with blank form; `q` → `Effect::Quit`. Empty list: `Enter` is a no-op.
- **D-032 CreateConnection.** Fields in order: Name, Host, Port, Database, User, Password. `Tab` next field, `BackTab` previous (wrap). Printable chars append to the focused field; `Backspace` pops. Port accepts digits only (other chars ignored). `Enter` validates: all fields non-empty except Password (may be empty), Port parses `u16` ≥ 1; on failure `Status::Error` with the first failing field (`error: <field> is required` / `error: port must be 1-65535`), stay on form. On success → `Effect::SaveConnection`; `Msg::Saved(Ok)` → reload list (`Effect::LoadConnections`), `Screen::ConnectionList`, cursor on the new row; `Msg::Saved(Err(DuplicateName))` → `error: name already exists`, stay on form. `Esc` → `Screen::ConnectionList`, form discarded.
- **D-033 Browser.** `Tab` toggles `Focus::Sidebar` ⇄ `Focus::Grid` (Grid only if `grid.is_some()`). Sidebar focus: `j`/`k`/`Up`/`Down` move (clamped); `Enter` → `Effect::Query { kind: Preview(t), sql }`. Grid focus: `h`/`Left`, `l`/`Right` move column cursor (clamped); `j`/`k`/`Up`/`Down` move row cursor (clamped); `s` cycles sort on the cursor column (D-052). `x` → `Screen::CustomSql` (keeps session and grid). `d` → `Effect::Disconnect`; `Msg::Disconnected` → `session = None`, `grid = None`, `sql_grid = None`, `Screen::ConnectionList`.
- **D-034 CustomSql.** Printable chars append to `sql_input`, `Backspace` pops, `Enter` → `Effect::Query { kind: Custom, sql }` (empty input → no-op). `Esc` → `Screen::Browser` (input and result retained). Result grid: `Up`/`Down` move row cursor only; **no sort, no column cursor**. Because letters type into the input, there is no `d`/`q` here; disconnect via `Esc` then `d`, exit via `Ctrl+C`.

### Exit codes and errors

- **D-040 Exit codes.** `0` normal quit (`q`, `Ctrl+C`). `2` usage/config: bad CLI args, store open failure, cannot create data dir — message on stderr as `error: <msg>`, before entering raw mode. `1` unexpected runtime failure (terminal IO error, panic) — terminal restored by a panic hook first.
- **D-041 Error surfacing.** Every `Err` reaching `App::update` becomes `Status::Error` (D-013). No error is swallowed; no `unwrap()`/`expect()` on IO results in `src/` outside `main.rs`'s terminal setup.

### Grid and sorting

- **D-050 Grid model.** `pub struct Grid { pub columns: Vec<String>, rows: Vec<Vec<Cell>>, pub col_cursor: usize, pub row_cursor: usize, pub sort: Option<SortState> }`, `pub struct SortState { pub column: usize, pub dir: SortDir }`, `pub enum SortDir { Asc, Desc }`. `Grid::from(ResultSet)`; `Grid::visible_rows(&self) -> Vec<&Vec<Cell>>` returns the sorted view (original order when `sort == None`); original row order is never mutated.
- **D-051 Comparison.** Per column: if every non-null cell parses as `f64` → numeric compare; else byte-wise `str` compare. Stable sort (`sort_by`); ties keep original order.
- **D-052 Null placement + cycling.** Asc: NULLs last. Desc: NULLs first (PostgreSQL semantics). `s` on cursor column: `None → Asc → Desc → None`; `s` on a different column starts at `Asc` and replaces the previous sort. Sort state resets when a new preview loads.
- **D-053 Sort is client-side** over the fetched ≤500 rows; no re-query.

### Layout (deterministic; snapshots depend on it)

- **D-060 Frame.** Snapshot terminal 100×30. Root layout: `[Min(0)] + [Length(1)]` → body + status line. Borders: `Borders::ALL`, plain. Titles wrapped in single spaces.
  - ConnectionList: body = one block, title ` pgtui - connections `, `List` rows `<name>  <display_dsn>`, `highlight_symbol` `> `; empty list shows the single line `No saved connections. Press n to create one.`. Help: `j/k move  Enter connect  n new  q quit`.
  - CreateConnection: block title ` New connection `; six lines `  <Label>: <value>`, focused line prefixed `> ` instead of two spaces, Password shown as `*` per char. Help: `Tab next  Enter save  Esc cancel`.
  - Browser: horizontal `[Length(30)] + [Min(0)]`. Sidebar block title ` Tables (<n>) `, rows `schema.name`, `highlight_symbol` `> `. Main block title ` <connection name> ` while `grid.is_none()` (body empty), else ` <schema.table>  <rows> rows  limit 500 `. The focused pane's title gets suffix `*` inside the brackets (e.g. ` Tables (3)* `). Help: `Tab focus  j/k move  Enter preview  h/l col  s sort  x sql  d disconnect`.
  - Grid (shared widget): ratatui `Table`; header row = column names; cursor column header wrapped as `[name]`; sorted column header suffixed ` ^` (asc) or ` v` (desc), e.g. `[balance ^]`; column width = `clamp(max(len(header)+4, max cell len), 4, 24)`; `highlight_symbol` `> ` on cursor row; `Cell::Null` → `NULL`. Custom SQL grid: same widget, no `[ ]` and no sort markers.
  - CustomSql: vertical `[Length(3)] + [Min(0)]`. Input block title ` SQL `, content `> <sql_input>`. Results block title ` Results ` while empty with body line `Type a query and press Enter`; after a result ` Results  <rows> rows `. Help: `Enter run  Up/Down rows  Esc back  Ctrl+C quit`.
- **D-061 Rendering entry point.** `ui::draw(frame: &mut Frame, app: &App)`; tests render with `ratatui::backend::TestBackend::new(100, 30)`.

### Test infrastructure (planner-shipped, protected)

- **D-070 `tests/support/mod.rs`** provides: `render_text(&App) -> String` (TestBackend buffer → lines, trailing spaces trimmed); `render_svg(&App) -> String` (one `<text>` per row, monospace, fg/bg from cell style, fixed font metrics — deterministic string); `svg_to_png(&str) -> Vec<u8>` (resvg + bundled DejaVu Sans Mono; used for `assert!(png.len() > 0 && dimensions == (100*cw, 30*ch))`, never byte-compared); `key(char)`, `key_code(KeyCode)`, `ctrl(char)` constructors; `temp_store() -> (TempDir, ConnectionStore)`; `pg_container() -> (Container, ConnParams)` starting `postgres:16-alpine` (version fixed by the verification agents) and applying `fixtures/seed.sql`; `fake_data::{tables(), preview(table) -> ResultSet}` mirroring `seed.sql` exactly.
- **D-071 Snapshot policy.** Text snapshots via `insta::assert_snapshot!("<name>", render_text(&app))`, SVG via `insta::assert_snapshot!("<name>__svg", render_svg(&app))`. Names fixed per task (listed in each task). All `.snap` files are shipped by the planner and protected. Gate runs tests with `INSTA_UPDATE=no INSTA_FORCE_PASS=0`; `*.snap.new` anywhere fails the gate (extra check). See §4 for the argument.
- **D-072 Seed fixture** `tests/fixtures/seed.sql`: schema `public`: `customers(id int PK, name text, balance numeric(10,2), signup_date date, note text NULL)` 5 rows (two NULL notes, balances 10.50, 250.00, -3.25, 99.99, 250.00 to exercise ties and negatives); `orders(id int PK, customer_id int, total numeric(10,2), status text)` 7 rows; `empty_table(id int PK)` 0 rows; schema `audit`: `events(id bigint PK, kind text, at timestamptz)` 3 rows. Sidebar order under D-025: `audit.events, public.customers, public.empty_table, public.orders`.

---

## 2. Task DAG

```text
TASK-101 ──> TASK-102 ──> TASK-103 ──> TASK-104 ──> TASK-105 ──> TASK-106
skeleton+     create       connect+      preview       custom        disconnect/exit
store+list    form         sidebar (PG)  grid+sort     SQL           + gallery
```

Strictly linear: each task's baseline is the previous task's reference solution (§5). Common to all tasks (stated once here, copied into each `task.md`):

- `kind: feature`; `verify: "/task/verify.sh"`.
- Preconditions (all tasks): **P-001** `cargo --version`; **P-002** `test -f crates/pgtui/tests/support/mod.rs`; **P-003** `git rev-parse --verify baseline`; tasks 103–106 add **P-004** `docker info >/dev/null`.
- Requirement present in every task: **R-00N (MUST)** implement the final design directly (template R-004); **R-00N (MUST NOT)** add/upgrade dependencies or edit `Cargo.toml`, `Cargo.lock`, `justfile`, `rust-toolchain.toml`, anything under `tests/`.
- verify.config common lines:

  ```bash
  BASE_REF="baseline"
  LINT_CMDS=("cargo fmt --all --check" "cargo clippy --workspace --all-targets -- -D warnings")
  DENIED_GLOBS=("Cargo.toml" "Cargo.lock" "crates/pgtui/Cargo.toml" "justfile" "rust-toolchain.toml" "crates/pgtui/tests/*" ".claude/*" "CLAUDE.md" "AGENTS.md")
  EXTRA_CHECKS=("no_snap_new")
  check_no_snap_new() { ! find crates -name '*.snap.new' -print | grep -q .; }
  ```

  Common forbidden patterns: `'#\[ignore\]|crates/pgtui'`, `'todo!\(|crates/pgtui/src'`, `'unimplemented!\(|crates/pgtui/src'`, `'dbg!\(|crates/pgtui/src'`, `'allow\((clippy|dead_code|unused)|crates/pgtui/src'`, `'INSTA_(UPDATE|FORCE_PASS)|crates/pgtui/src'`, `'TODO\(TASK-1..\)'` (per task id).
- Every focused command is prefixed `INSTA_UPDATE=no INSTA_FORCE_PASS=0` and uses `cargo test -p pgtui --test <file>` so a compile failure in one trusted test file cannot mask another.
- Checklist leaves 1.x and 4.x are the template's; only 2.x/3.x vary. Leaf counts below (13–16) are within 5–20.

### TASK-101 — Saved PostgreSQL connections are listed from a Turso store in a ratatui shell

- **Goal:** Running `pgtui` opens a TUI whose first screen lists the connections persisted in the local Turso SQLite file and exits cleanly with `q`.
- **In scope:** `store/` (open, schema, list, insert, duplicate error); `app.rs` state core with `Screen`, `Msg`, `Effect`, list navigation and quit; `keys.rs` for ConnectionList + global; `ui/connection_list.rs`, `ui/status.rs`, `ui/mod.rs`; `runtime.rs` for `LoadConnections`/`Quit`; `main.rs` CLI, terminal lifecycle, exit codes 0/2. Unit tests for `keys` and store path resolution.
- **Out of scope:** create form (`n` is a no-op here), connecting (`Enter` no-op), any PG code, keyring, config file, themes.
- **Requirements:** R-001 store per D-020..D-022 (schema idempotent, list sorted by name, `DuplicateName` on unique violation). R-002 app core per D-010..D-013 with ConnectionList keys per D-030/D-031. R-003 render per D-060 ConnectionList incl. empty state. R-004 CLI/exit codes per D-040; `--db` unwritable → exit 2, stderr `error:`; `--version` exit 0. R-005 MUST NOT block on any DB call before `--help`/`--version` handling. R-006 final design / no deps (common).
- **Fixed decisions referenced:** D-001..D-005, D-010..D-013, D-020..D-022, D-030, D-031, D-040, D-041, D-060, D-061, D-070, D-071.
- **Baseline command:** `cargo test -p pgtui --test store_test` → expected `error[E0432]: unresolved import` (lib has no `store` items).
- **AC table:**

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given a fresh path, when the store opens, inserts two connections and reopens, then `list()` returns both sorted by name with `created_at` RFC 3339. | `cargo test -p pgtui --test store_test` | exit 0, `4 passed` (`open_creates_schema`, `insert_then_list_sorted_by_name`, `reopen_persists`, `display_dsn_hides_password`) |
| AC-002 | Given a saved name, when inserted again, then `StoreError::DuplicateName`. | `cargo test -p pgtui --test store_test duplicate_name_rejected` | exit 0, `1 passed` |
| AC-003 | Given no connections, when rendered, then the empty-state screen matches the snapshot. | `cargo test -p pgtui --test screen_connection_list_test screen__connection_list_empty` | exit 0, `2 passed` (text + svg) |
| AC-004 | Given two connections with cursor on the second, when rendered, then matches snapshot. | `cargo test -p pgtui --test screen_connection_list_test screen__connection_list_two` | exit 0, `2 passed` |
| AC-005 | Given the list screen, when `j`/`k`/`q`/`Ctrl+C` are pressed, then cursor clamps and quit effects are emitted. | `cargo test -p pgtui --test app_connection_list_test` | exit 0, `5 passed` |
| AC-006 | Given an unwritable `--db`, when `pgtui` starts, then exit 2 with `error:` on stderr; `--version` exits 0. | `cargo test -p pgtui --test cli_test` | exit 0, `3 passed` |

- **expected_paths:** `crates/pgtui/src/*`. **protected_paths:** `crates/pgtui/tests/` (whole dir), `Cargo.toml`, `Cargo.lock`, `crates/pgtui/Cargo.toml`, `justfile`, `rust-toolchain.toml`, `CLAUDE.md`.
- **Checklist (16 leaves):**

```markdown
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-003` pass — evidence: each command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test store_test` fails to compile with unresolved `pgtui::store` imports.
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Turso connection store (`R-001`, `D-020..D-022`) — evidence: `cargo test -p pgtui --test store_test` exits 0.
        - [ ] **2.1.1** `ConnectionStore::open` creates the D-021 schema idempotently — evidence: `cargo test -p pgtui --test store_test open_creates_schema` → `1 passed`.
        - [ ] **2.1.2** `insert` returns the saved row and `list` orders by name — evidence: `cargo test -p pgtui --test store_test insert_then_list_sorted_by_name` → `1 passed`.
        - [ ] **2.1.3** Unique-name violation maps to `StoreError::DuplicateName` — evidence: `cargo test -p pgtui --test store_test duplicate_name_rejected` → `1 passed`.
    - [ ] **2.2** App core and key mapping (`R-002`, `D-010`, `D-011`, `D-030`, `D-031`) — evidence: `cargo test -p pgtui --test app_connection_list_test` exits 0.
        - [ ] **2.2.1** `Msg::Connections` populates the list and clamps the cursor — evidence: `cargo test -p pgtui --test app_connection_list_test connections_msg_populates_list` → `1 passed`.
        - [ ] **2.2.2** `j`/`k` clamp at both ends; `q` and `Ctrl+C` emit `Effect::Quit` — evidence: `cargo test -p pgtui --test app_connection_list_test` → `5 passed`.
    - [ ] **2.3** ConnectionList and status line render per `D-060` (`R-003`) — evidence: `cargo test -p pgtui --test screen_connection_list_test` exits 0, `4 passed`.
    - [ ] **2.4** CLI, store path precedence and exit codes (`R-004`, `R-005`, `D-020`, `D-040`) — evidence: `cargo test -p pgtui --test cli_test` exits 0, `3 passed`.
    - [ ] **2.5** Unit tests for `keys::action_for` and store path resolution exist in `src/` — evidence: `cargo test -p pgtui --lib -- --list | grep -c 'keys::tests\|store::tests'` prints ≥ 4.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001` and `AC-002` — evidence: `cargo test -p pgtui --test store_test` → `5 passed`.
    - [ ] **3.2** `AC-003` and `AC-004` — evidence: `INSTA_UPDATE=no cargo test -p pgtui --test screen_connection_list_test` → `4 passed`.
    - [ ] **3.3** `AC-005` — evidence: `cargo test -p pgtui --test app_connection_list_test` → `5 passed`.
    - [ ] **3.4** `AC-006` — evidence: `cargo test -p pgtui --test cli_test` → `3 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only `crates/pgtui/src/*` changed — evidence: `git status --porcelain` shows only those paths.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full output in the transcript.
```

- **verify.config (task-specific lines):**

```bash
FOCUSED_CMDS=("cargo test -p pgtui --test store_test" "INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_connection_list_test" "cargo test -p pgtui --test app_connection_list_test" "cargo test -p pgtui --test cli_test")
REGRESSION_CMDS=("INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui")
FORBIDDEN_PATTERNS=(<common> 'TODO\(TASK-101\)' 'tokio_postgres|crates/pgtui/src')   # no PG code yet
FORBIDDEN_PATHS=("crates/pgtui/src/db/postgres.rs")
REQUIRED_PATHS=("crates/pgtui/src/store/mod.rs" "crates/pgtui/src/app.rs" "crates/pgtui/src/keys.rs" "crates/pgtui/src/runtime.rs" "crates/pgtui/src/ui/connection_list.rs" "crates/pgtui/src/ui/status.rs")
ALLOWED_GLOBS=("crates/pgtui/src/*")
```

### TASK-102 — A new connection can be created from the TUI and appears in the list

- **Goal:** Pressing `n` opens a six-field form whose valid submission persists a connection and returns to the list with the new row selected.
- **In scope:** `CreateForm` in `app.rs`, `keys.rs` CreateConnection mapping (D-032), validation, `Effect::SaveConnection` + `Msg::Saved` handling, `runtime.rs` execution, `ui/create_form.rs`. Unit tests for validation.
- **Out of scope:** editing/deleting connections, connecting, password masking toggle, keyring.
- **Requirements:** R-001 form fields, focus cycling and editing per D-032. R-002 validation messages exactly per D-032; invalid submit stays on form. R-003 successful save reloads the list, selects the new row, returns to ConnectionList; `DuplicateName` shows `error: name already exists` and keeps the form. R-004 `Esc` discards. R-005 render per D-060 CreateConnection with `*` masking. R-006 common.
- **Fixed decisions referenced:** D-010..D-013, D-021, D-022, D-032, D-041, D-060, D-071.
- **Baseline command:** `cargo test -p pgtui --test app_create_form_test` → compile error (no `CreateForm`, `n` is a no-op).
- **AC table:**

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given the list, when `n` then typing into fields with `Tab`/`BackTab`, then the form state matches the typed values and Port ignores non-digits. | `cargo test -p pgtui --test app_create_form_test editing` | exit 0, `4 passed` |
| AC-002 | Given a form with an empty Host or Port 0, when `Enter`, then `Status::Error` names the field and screen stays `CreateConnection`. | `cargo test -p pgtui --test app_create_form_test validation` | exit 0, `3 passed` |
| AC-003 | Given a valid form, when `Enter` and the runtime executes the effect against a temp store, then the store has the row and the app is on `ConnectionList` with the cursor on it. | `cargo test -p pgtui --test runtime_create_test save_roundtrip` | exit 0, `1 passed` |
| AC-004 | Given an existing name, when submitted again, then `error: name already exists` and the form is kept. | `cargo test -p pgtui --test runtime_create_test duplicate_keeps_form` | exit 0, `1 passed` |
| AC-005 | Given a blank form and a filled form (Password focused), when rendered, then both match snapshots. | `cargo test -p pgtui --test screen_create_form_test` | exit 0, `4 passed` (`screen__create_form_blank`, `screen__create_form_filled`, + svg) |
| AC-006 | Given a saved form, when rendered after `Msg::Saved(Ok)`, then the list snapshot with the new row selected matches. | `cargo test -p pgtui --test screen_create_form_test screen__create_form_saved_list` | exit 0, `2 passed` |

- **expected_paths:** `crates/pgtui/src/app.rs`, `crates/pgtui/src/keys.rs`, `crates/pgtui/src/runtime.rs`, `crates/pgtui/src/ui/create_form.rs`, `crates/pgtui/src/ui/mod.rs`. **protected_paths:** as 101 plus `crates/pgtui/src/store/` (store API is frozen from here on).
- **Checklist (14 leaves):**

```markdown
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-003` pass — evidence: each command exits 0.
    - [ ] **1.2** Baseline failure recorded — evidence: `cargo test -p pgtui --test app_create_form_test` fails to compile (`CreateForm` missing).
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Form state and editing keys (`R-001`, `D-032`) — evidence: `cargo test -p pgtui --test app_create_form_test editing` → `4 passed`.
        - [ ] **2.1.1** `n` enters `Screen::CreateConnection` with a blank form focused on Name — evidence: `cargo test -p pgtui --test app_create_form_test editing_n_opens_blank_form` → `1 passed`.
        - [ ] **2.1.2** `Tab`/`BackTab` wrap through the six fields; chars append, `Backspace` pops, Port digits only — evidence: `cargo test -p pgtui --test app_create_form_test editing_` → `4 passed`.
    - [ ] **2.2** Validation and error messages (`R-002`, `D-032`, `D-013`) — evidence: `cargo test -p pgtui --test app_create_form_test validation` → `3 passed`.
    - [ ] **2.3** Save effect, reload, cursor placement, duplicate handling (`R-003`, `R-004`) — evidence: `cargo test -p pgtui --test runtime_create_test` → `3 passed`.
        - [ ] **2.3.1** `runtime::execute(SaveConnection)` inserts and replies `Msg::Saved` — evidence: `cargo test -p pgtui --test runtime_create_test save_roundtrip` → `1 passed`.
        - [ ] **2.3.2** `Msg::Saved(Err(DuplicateName))` keeps the form with the error status; `Esc` discards — evidence: `cargo test -p pgtui --test runtime_create_test duplicate_keeps_form esc_discards` → `2 passed`.
    - [ ] **2.4** Form renders per `D-060` with masked password (`R-005`) — evidence: `INSTA_UPDATE=no cargo test -p pgtui --test screen_create_form_test` → `6 passed`.
    - [ ] **2.5** Unit tests for `CreateForm::validate` exist in `src/app.rs` — evidence: `cargo test -p pgtui --lib -- --list | grep -c 'app::tests::validate'` prints ≥ 3.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001`, `AC-002` — evidence: `cargo test -p pgtui --test app_create_form_test` → `7 passed`.
    - [ ] **3.2** `AC-003`, `AC-004` — evidence: `cargo test -p pgtui --test runtime_create_test` → `3 passed`.
    - [ ] **3.3** `AC-005`, `AC-006` — evidence: `INSTA_UPDATE=no cargo test -p pgtui --test screen_create_form_test` → `6 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only the five expected files changed — evidence: `git status --porcelain`.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full output in the transcript.
```

- **verify.config:** `FOCUSED_CMDS` = the three test files above (insta env on the screen one); `REGRESSION_CMDS=("INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui")`; `REQUIRED_PATHS=("crates/pgtui/src/ui/create_form.rs")`; `ALLOWED_GLOBS=("crates/pgtui/src/app.rs" "crates/pgtui/src/keys.rs" "crates/pgtui/src/runtime.rs" "crates/pgtui/src/ui/create_form.rs" "crates/pgtui/src/ui/mod.rs")`; `DENIED_GLOBS` += `"crates/pgtui/src/store/*"`; forbidden `'TODO\(TASK-102\)'`.

### TASK-103 — Selecting a connection connects to PostgreSQL and lists its tables in a sidebar

- **Goal:** `Enter` on a saved connection opens the Browser screen with every user table in the left sidebar and an empty main body; connection failure is shown on the list screen.
- **In scope:** `db/mod.rs` types, `db/postgres.rs` `connect` + `list_tables` (D-023..D-025 tables query only), `Effect::Connect` execution with timeout, `Msg::Connected` handling, Browser sidebar keys (`j/k`, `Tab` no-op without grid), `ui/browser.rs` sidebar + empty main, `SessionView`. Testcontainers integration test for real table listing.
- **Out of scope:** previewing a table (`Enter` in sidebar is a no-op), sorting, custom SQL, disconnect (`d` no-op until 106; `Ctrl+C` still quits), TLS.
- **Requirements:** R-001 `PgSession::connect(&ConnParams)` per D-024 with 5 s timeout; `PgSession::list_tables()` per D-025 returning `Vec<TableRef>` in server order. R-002 `Msg::Connected(Ok(tables))` → `Screen::Browser`, `session = Some(SessionView{name, tables, sidebar_cursor: 0, focus: Sidebar})`. R-003 `Msg::Connected(Err)` → stay on list, `Status::Error`. R-004 sidebar navigation clamps (D-033). R-005 render per D-060 Browser with empty body and focus marker. R-006 MUST NOT `#[ignore]` PG tests or skip them when Docker is absent (precondition P-004 guarantees Docker). R-007 common.
- **Fixed decisions referenced:** D-010..D-013, D-023..D-026, D-033, D-041, D-060, D-070 (`pg_container`, `fake_data`), D-071, D-072.
- **Baseline command:** `cargo test -p pgtui --test pg_connect_test` → compile error (no `pgtui::db::postgres`).
- **AC table:**

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given a seeded container, when `connect` then `list_tables`, then the four seed tables in D-072 order. | `cargo test -p pgtui --test pg_connect_test lists_seed_tables` | exit 0, `1 passed` |
| AC-002 | Given a closed port, when `connect`, then `Err(DbError::Connect)` within 6 s. | `cargo test -p pgtui --test pg_connect_test refused_port_errors_fast` | exit 0, `1 passed` |
| AC-003 | Given the list with a connection, when `Enter` then `Msg::Connected(Ok(tables))`, then Browser with cursor 0 and Sidebar focus; `Msg::Connected(Err)` keeps the list with `error:`. | `cargo test -p pgtui --test app_browser_test` | exit 0, `5 passed` |
| AC-004 | Given the runtime with the container's params, when `Effect::Connect` executes, then reply is `Connected(Ok)` with the seed tables (end-to-end through `runtime::execute`). | `cargo test -p pgtui --test pg_runtime_connect_test` | exit 0, `1 passed` |
| AC-005 | Given four fake tables, when rendered with cursor on `public.customers`, then Browser snapshot with empty body matches. | `cargo test -p pgtui --test screen_browser_test screen__browser_sidebar_empty_body` | exit 0, `2 passed` |
| AC-006 | Given real PG table list, when compared with `fake_data::tables()`, then equal (keeps fake honest). | `cargo test -p pgtui --test pg_connect_test fake_tables_match_pg` | exit 0, `1 passed` |

- **expected_paths:** `crates/pgtui/src/db/*`, `crates/pgtui/src/app.rs`, `crates/pgtui/src/keys.rs`, `crates/pgtui/src/runtime.rs`, `crates/pgtui/src/ui/browser.rs`, `crates/pgtui/src/ui/mod.rs`, `crates/pgtui/src/lib.rs`. **protected_paths:** as 102.
- **Checklist (14 leaves):**

```markdown
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-004` pass — evidence: each command exits 0 (incl. `docker info`).
    - [ ] **1.2** Baseline failure recorded — evidence: `cargo test -p pgtui --test pg_connect_test` fails to compile (`pgtui::db::postgres` missing).
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** PostgreSQL session (`R-001`, `D-023..D-025`) — evidence: `cargo test -p pgtui --test pg_connect_test` → `3 passed`.
        - [ ] **2.1.1** `PgSession::connect` builds the D-024 config, uses `NoTls`, times out at 5 s — evidence: `cargo test -p pgtui --test pg_connect_test refused_port_errors_fast` → `1 passed`.
        - [ ] **2.1.2** `list_tables` runs the D-025 query via `simple_query` — evidence: `cargo test -p pgtui --test pg_connect_test lists_seed_tables` → `1 passed`.
    - [ ] **2.2** App handles `Connected` and sidebar keys (`R-002..R-004`, `D-033`) — evidence: `cargo test -p pgtui --test app_browser_test` → `5 passed`.
        - [ ] **2.2.1** `Enter` on the list emits `Effect::Connect(selected)`; `Connected(Ok)` enters Browser — evidence: `cargo test -p pgtui --test app_browser_test enter_connects` → `2 passed`.
        - [ ] **2.2.2** `Connected(Err)` stays on the list with `Status::Error`; `j`/`k` clamp in the sidebar — evidence: `cargo test -p pgtui --test app_browser_test connect_error sidebar_` → `3 passed`.
    - [ ] **2.3** `runtime::execute(Connect)` connects, lists, replies (`R-001`, `D-012`) — evidence: `cargo test -p pgtui --test pg_runtime_connect_test` → `1 passed`.
    - [ ] **2.4** Browser renders sidebar + empty body + focus marker per `D-060` (`R-005`) — evidence: `INSTA_UPDATE=no cargo test -p pgtui --test screen_browser_test` → `2 passed`.
    - [ ] **2.5** Unit test for identifier quoting helper exists in `src/db/mod.rs` — evidence: `cargo test -p pgtui --lib db::tests::quote_ident` → `≥ 1 passed`.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001`, `AC-002`, `AC-006` — evidence: `cargo test -p pgtui --test pg_connect_test` → `3 passed`.
    - [ ] **3.2** `AC-003`, `AC-004` — evidence: `cargo test -p pgtui --test app_browser_test --test pg_runtime_connect_test` → `6 passed`.
    - [ ] **3.3** `AC-005` — evidence: `INSTA_UPDATE=no cargo test -p pgtui --test screen_browser_test` → `2 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only expected paths changed — evidence: `git status --porcelain`.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full output in the transcript.
```

- **verify.config:** `FOCUSED_CMDS` = `pg_connect_test`, `app_browser_test`, `pg_runtime_connect_test`, `screen_browser_test` (insta env); `REGRESSION_CMDS=("INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui")`; `REQUIRED_PATHS=("crates/pgtui/src/db/mod.rs" "crates/pgtui/src/db/postgres.rs" "crates/pgtui/src/ui/browser.rs")`; `ALLOWED_GLOBS` = expected_paths; forbidden += `'TODO\(TASK-103\)'`, `'sslmode|crates/pgtui/src'`, `'query_raw|query_one|crates/pgtui/src'` (enforces simple-query path).

### TASK-104 — A selected table is previewed in a sortable grid

- **Goal:** `Enter` on a sidebar table shows its first 500 rows in the main pane, and `s` sorts the grid by the cursor column ascending, descending, then unsorted.
- **In scope:** `grid.rs` (D-050..D-053), `PgSession::query(sql) -> ResultSet` via simple-query (rows only; command-complete handling minimal), `Effect::Query{Preview}` + `Msg::QueryDone{Preview}`, grid keys (`Tab` focus, `h/l/j/k`, `s`), `ui/grid.rs`, main-pane title, unit tests for comparison and null placement.
- **Out of scope:** custom SQL (`x` no-op), server-side sort, pagination beyond 500, column resizing, disconnect.
- **Requirements:** R-001 preview SQL exactly per D-025 with quoted identifiers; `PREVIEW_LIMIT` const. R-002 `Grid` per D-050; `visible_rows` never mutates source order. R-003 sort per D-051/D-052 (numeric detection, NULLS LAST asc / FIRST desc, stable, cycle, reset on new preview). R-004 keys per D-033 grid focus; `Tab` is a no-op while `grid.is_none()`. R-005 render per D-060 grid (`[col]`, ` ^`/` v`, widths, `NULL`). R-006 `QueryDone(Err)` → `Status::Error`, previous grid retained. R-007 common.
- **Fixed decisions referenced:** D-025, D-026, D-033, D-041, D-050..D-053, D-060, D-070, D-071, D-072.
- **Baseline command:** `cargo test -p pgtui --test grid_sort_test` → compile error (`pgtui::grid` missing).
- **AC table:**

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given `fake_data::preview(customers)`, when sorted by `balance` asc/desc, then numeric order with NULL placement per D-052 and stable ties. | `cargo test -p pgtui --test grid_sort_test` | exit 0, `6 passed` |
| AC-002 | Given the seeded container, when `query` runs the preview SQL for each seed table, then row counts equal `fake_data::preview(t)` and cells are equal (incl. `Null`). | `cargo test -p pgtui --test pg_preview_test` | exit 0, `3 passed` (`preview_matches_fake`, `empty_table_zero_rows`, `limit_applied_on_600_row_table`) |
| AC-003 | Given Browser with sidebar focus, when `Enter`, then `Effect::Query{Preview}` with the exact SQL string; on `QueryDone(Ok)` focus moves to Grid and sort is `None`. | `cargo test -p pgtui --test app_preview_test` | exit 0, `6 passed` |
| AC-004 | Given a grid, when `s` pressed 1/2/3 times on a column, then `Asc`/`Desc`/`None`; `s` on another column → `Asc` there. | `cargo test -p pgtui --test app_preview_test sort_cycle` | exit 0, `2 passed` |
| AC-005 | Given fake customers loaded, when rendered unsorted, sorted asc by balance, sorted desc by balance, then three snapshots match (`screen__preview_unsorted`, `screen__preview_sorted_asc`, `screen__preview_sorted_desc`). | `cargo test -p pgtui --test screen_preview_test` | exit 0, `6 passed` |
| AC-006 | Given `QueryDone(Err)`, when handled, then status is `error: ...` and the previous grid is unchanged. | `cargo test -p pgtui --test app_preview_test query_error_keeps_grid` | exit 0, `1 passed` |

- **expected_paths:** `crates/pgtui/src/grid.rs`, `crates/pgtui/src/db/*`, `crates/pgtui/src/app.rs`, `crates/pgtui/src/keys.rs`, `crates/pgtui/src/runtime.rs`, `crates/pgtui/src/ui/browser.rs`, `crates/pgtui/src/ui/grid.rs`, `crates/pgtui/src/ui/mod.rs`, `crates/pgtui/src/lib.rs`. **protected_paths:** as 102.
- **Checklist (15 leaves):**

```markdown
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-004` pass — evidence: each command exits 0.
    - [ ] **1.2** Baseline failure recorded — evidence: `cargo test -p pgtui --test grid_sort_test` fails to compile (`pgtui::grid` missing).
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Grid model and sorting (`R-002`, `R-003`, `D-050..D-053`) — evidence: `cargo test -p pgtui --test grid_sort_test` → `6 passed`.
        - [ ] **2.1.1** Numeric-vs-string column detection and comparator — evidence: `cargo test -p pgtui --test grid_sort_test numeric_ string_` → `2 passed`.
        - [ ] **2.1.2** NULL placement and stable ties — evidence: `cargo test -p pgtui --test grid_sort_test nulls_ stable_` → `3 passed`.
        - [ ] **2.1.3** Unit tests for the comparator in `src/grid.rs` — evidence: `cargo test -p pgtui --lib grid::tests` → `≥ 3 passed`.
    - [ ] **2.2** `PgSession::query` returns all-text `ResultSet` and preview SQL matches `D-025` (`R-001`) — evidence: `cargo test -p pgtui --test pg_preview_test` → `3 passed`.
    - [ ] **2.3** App preview flow and grid keys (`R-004`, `R-006`, `D-033`) — evidence: `cargo test -p pgtui --test app_preview_test` → `9 passed`.
        - [ ] **2.3.1** `Enter` emits `Effect::Query{Preview}`; `QueryDone(Ok)` installs the grid and focuses it — evidence: `cargo test -p pgtui --test app_preview_test enter_ query_ok` → `3 passed`.
        - [ ] **2.3.2** `h/l/j/k` clamp; `s` cycles; `Tab` toggles focus only with a grid — evidence: `cargo test -p pgtui --test app_preview_test cursor_ sort_cycle tab_` → `5 passed`.
    - [ ] **2.4** Grid widget and main-pane title per `D-060` (`R-005`) — evidence: `INSTA_UPDATE=no cargo test -p pgtui --test screen_preview_test` → `6 passed`.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001` — evidence: `cargo test -p pgtui --test grid_sort_test` → `6 passed`.
    - [ ] **3.2** `AC-002` — evidence: `cargo test -p pgtui --test pg_preview_test` → `3 passed`.
    - [ ] **3.3** `AC-003`, `AC-004`, `AC-006` — evidence: `cargo test -p pgtui --test app_preview_test` → `9 passed`.
    - [ ] **3.4** `AC-005` — evidence: `INSTA_UPDATE=no cargo test -p pgtui --test screen_preview_test` → `6 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only expected paths changed — evidence: `git status --porcelain`.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full output in the transcript.
```

- **verify.config:** `FOCUSED_CMDS` = the four files; `REGRESSION_CMDS=("INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui")`; `REQUIRED_PATHS=("crates/pgtui/src/grid.rs" "crates/pgtui/src/ui/grid.rs")`; forbidden += `'TODO\(TASK-104\)'`, `'ORDER BY|crates/pgtui/src/app.rs crates/pgtui/src/grid.rs'` (no server-side sort), `'sort_unstable|crates/pgtui/src'`. Note `pg_preview_test::limit_applied_on_600_row_table` creates its own 600-row table inside the test (not in `seed.sql`) so the seed stays small.

### TASK-105 — Custom SQL runs from a dedicated screen into the same grid

- **Goal:** `x` in the Browser opens a SQL screen whose input, on `Enter`, runs one statement and shows its rows in a non-sortable grid.
- **In scope:** CustomSql keys (D-034), `sql_input`/`sql_grid` state, `QueryKind::Custom` handling incl. `MultiStatement`, command-complete and 500-row cap per D-025, `ui/custom_sql.rs`, grid widget flag for "plain" mode (no `[ ]`/sort markers).
- **Out of scope:** history, multi-line editor, autocomplete, cancel running query, disconnect.
- **Requirements:** R-001 `x` → `Screen::CustomSql` keeping session and grid; `Esc` → `Browser` retaining input and result. R-002 editing per D-034; `Enter` on empty input is a no-op. R-003 custom SQL sent verbatim minus trailing `;`; `MultiStatement` → `error: one statement at a time`; `CommandComplete` → `Status::Info("ok: <n> rows affected")` and `sql_grid = None`; rows capped at 500 with info status. R-004 result grid has no sort and no column cursor; `s`/`h`/`l` type into the input instead. R-005 render per D-060 CustomSql (empty and with results). R-006 common.
- **Fixed decisions referenced:** D-010..D-013, D-025, D-026, D-034, D-041, D-050, D-060, D-071, D-072.
- **Baseline command:** `cargo test -p pgtui --test app_custom_sql_test` → `x` does nothing; assertion `screen == CustomSql` fails.
- **AC table:**

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given Browser, when `x`, typing, `Backspace`, `Esc`, `x`, then screen transitions and `sql_input` retained. | `cargo test -p pgtui --test app_custom_sql_test navigation` | exit 0, `3 passed` |
| AC-002 | Given `select 1 as a;`, when `Enter`, then `Effect::Query{Custom, "select 1 as a"}`; empty input emits nothing. | `cargo test -p pgtui --test app_custom_sql_test enter_` | exit 0, `2 passed` |
| AC-003 | Given the container, when `SELECT name FROM customers WHERE note IS NULL ORDER BY id` runs, then two rows; `UPDATE ... ` returns `CommandComplete` info; `select 1; select 2` → `MultiStatement`; a 600-row `generate_series` is capped at 500. | `cargo test -p pgtui --test pg_custom_sql_test` | exit 0, `4 passed` |
| AC-004 | Given a custom grid, when `s` is pressed, then `sql_input` ends with `s` and `sql_grid.sort` stays `None`. | `cargo test -p pgtui --test app_custom_sql_test no_sort_in_custom_grid` | exit 0, `1 passed` |
| AC-005 | Given empty SQL screen and after a fake result, when rendered, then `screen__custom_sql_empty` and `screen__custom_sql_results` match. | `cargo test -p pgtui --test screen_custom_sql_test` | exit 0, `4 passed` |

- **expected_paths:** `crates/pgtui/src/app.rs`, `crates/pgtui/src/keys.rs`, `crates/pgtui/src/runtime.rs`, `crates/pgtui/src/db/*`, `crates/pgtui/src/ui/custom_sql.rs`, `crates/pgtui/src/ui/grid.rs`, `crates/pgtui/src/ui/mod.rs`. **protected_paths:** as 102 plus `crates/pgtui/src/grid.rs` (model frozen).
- **Checklist (13 leaves):**

```markdown
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-004` pass — evidence: each command exits 0.
    - [ ] **1.2** Baseline failure recorded — evidence: `cargo test -p pgtui --test app_custom_sql_test` → `navigation_x_opens_sql_screen` fails (`screen == Browser`).
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Screen transitions and input editing (`R-001`, `R-002`, `D-034`) — evidence: `cargo test -p pgtui --test app_custom_sql_test navigation enter_` → `5 passed`.
    - [ ] **2.2** Custom query execution semantics (`R-003`, `D-025`) — evidence: `cargo test -p pgtui --test pg_custom_sql_test` → `4 passed`.
        - [ ] **2.2.1** Trailing `;` stripped; multi-row-description → `DbError::MultiStatement` — evidence: `cargo test -p pgtui --test pg_custom_sql_test multi_statement_rejected trailing_semicolon` → `2 passed`.
        - [ ] **2.2.2** `CommandComplete` → info status, `sql_grid = None`; rows capped at 500 with info — evidence: `cargo test -p pgtui --test pg_custom_sql_test command_complete capped_` → `2 passed`.
    - [ ] **2.3** Custom grid is plain: no sort, no column cursor (`R-004`) — evidence: `cargo test -p pgtui --test app_custom_sql_test no_sort_in_custom_grid` → `1 passed`.
    - [ ] **2.4** CustomSql screen renders per `D-060` (`R-005`) — evidence: `INSTA_UPDATE=no cargo test -p pgtui --test screen_custom_sql_test` → `4 passed`.
    - [ ] **2.5** Unit test for `strip_trailing_semicolon` in `src/db/mod.rs` — evidence: `cargo test -p pgtui --lib db::tests::strip_trailing` → `≥ 1 passed`.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001`, `AC-002`, `AC-004` — evidence: `cargo test -p pgtui --test app_custom_sql_test` → `6 passed`.
    - [ ] **3.2** `AC-003` — evidence: `cargo test -p pgtui --test pg_custom_sql_test` → `4 passed`.
    - [ ] **3.3** `AC-005` — evidence: `INSTA_UPDATE=no cargo test -p pgtui --test screen_custom_sql_test` → `4 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only expected paths changed — evidence: `git status --porcelain`.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full output in the transcript.
```

- **verify.config:** `FOCUSED_CMDS` = three files; regression full; `REQUIRED_PATHS=("crates/pgtui/src/ui/custom_sql.rs")`; `DENIED_GLOBS` += `"crates/pgtui/src/grid.rs"`; forbidden += `'TODO\(TASK-105\)'`.

### TASK-106 — Disconnecting returns to the list, exiting restores the terminal, and every screen renders to SVG/PNG

- **Goal:** `d` cleanly closes the PG session and returns to the connection list; `q`/`Ctrl+C` exit with code 0 from any state; `cargo run --bin gallery` writes one SVG and one PNG per screen into `docs/screens/`.
- **In scope:** `Effect::Disconnect` execution (drop session, await connection task end), `Msg::Disconnected` state reset, `Ctrl+C` from Browser/CustomSql disconnects before quitting, panic hook restoring the terminal, `src/bin/gallery.rs` using `tests/support`-equivalent rendering from `fake_data` (the render helpers are re-exported for the bin via a `pgtui::render` module the planner ships in `src/render.rs` at 101 — see §3), `docs/screens/` output, README section listing the screens.
- **Out of scope:** new screens, reconnect, theming, CI.
- **Requirements:** R-001 `d` in Browser → `Effect::Disconnect`; runtime drops `PgSession` and replies `Disconnected`; app resets per D-033. R-002 `Ctrl+C` while connected emits `[Disconnect, Quit]` in that order (D-030). R-003 exit code 0 on both quit paths; terminal restored (raw mode off, alternate screen left) on normal exit and on panic (D-040). R-004 `gallery` renders the ten named screens (D-071 names across 101–105) to `docs/screens/<name>.svg` and `.png`, deterministic SVG bytes across runs. R-005 MUST NOT duplicate render code: the bin uses `pgtui::render`. R-006 common.
- **Fixed decisions referenced:** D-002 (`bin/gallery.rs`), D-012, D-030, D-033, D-040, D-041, D-070, D-071.
- **Baseline command:** `cargo test -p pgtui --test app_disconnect_test` → `d_disconnects` fails (`d` is a no-op).
- **AC table:**

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given Browser with a grid, when `d` then `Msg::Disconnected`, then `ConnectionList`, `session/grid/sql_grid` are `None`, cursor preserved. | `cargo test -p pgtui --test app_disconnect_test` | exit 0, `4 passed` |
| AC-002 | Given the container, when runtime executes `Connect` then `Disconnect`, then the PG backend count for the app's `application_name` drops to 0 (checked via a second admin connection: `SELECT count(*) FROM pg_stat_activity WHERE application_name = 'pgtui'`). | `cargo test -p pgtui --test pg_disconnect_test` | exit 0, `1 passed` |
| AC-003 | Given `Ctrl+C` in CustomSql while connected, when handled, then effects are exactly `[Disconnect, Quit]`. | `cargo test -p pgtui --test app_disconnect_test ctrl_c_disconnects_then_quits` | exit 0, `1 passed` |
| AC-004 | Given the binary under a pty with an empty store, when `q` is sent, then exit 0 and the final terminal output contains the leave-alternate-screen sequence. | `cargo test -p pgtui --test cli_exit_test` | exit 0, `2 passed` (`q_exits_zero`, `ctrl_c_exits_zero`) |
| AC-005 | Given `cargo run -p pgtui --bin gallery -- --out target/gallery`, when run twice, then 20 files exist (10 svg + 10 png), every SVG equals the protected snapshot's `__svg` body, and both runs' SVGs are byte-identical. | `cargo test -p pgtui --test gallery_test` | exit 0, `3 passed` |
| AC-006 | Given the whole suite, when run, then every snapshot from 101–105 still passes (completeness). | `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test 'screen_*'` | exit 0, `28 passed` |

- **expected_paths:** `crates/pgtui/src/app.rs`, `crates/pgtui/src/keys.rs`, `crates/pgtui/src/runtime.rs`, `crates/pgtui/src/main.rs`, `crates/pgtui/src/db/postgres.rs`, `crates/pgtui/src/bin/gallery.rs`, `docs/screens/*`, `README.md`. **protected_paths:** as 105 plus `crates/pgtui/src/render.rs`.
- **Checklist (14 leaves):**

```markdown
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-004` pass — evidence: each command exits 0.
    - [ ] **1.2** Baseline failure recorded — evidence: `cargo test -p pgtui --test app_disconnect_test` → `d_disconnects` fails.
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Disconnect flow (`R-001`, `R-002`, `D-030`, `D-033`) — evidence: `cargo test -p pgtui --test app_disconnect_test` → `5 passed`.
        - [ ] **2.1.1** `d` emits `Effect::Disconnect`; `Disconnected` resets session state — evidence: `cargo test -p pgtui --test app_disconnect_test d_disconnects disconnected_resets` → `2 passed`.
        - [ ] **2.1.2** `Ctrl+C` while connected yields `[Disconnect, Quit]` — evidence: `cargo test -p pgtui --test app_disconnect_test ctrl_c_disconnects_then_quits` → `1 passed`.
    - [ ] **2.2** Runtime closes the PG session and sets `application_name = 'pgtui'` (`R-001`, `D-024`) — evidence: `cargo test -p pgtui --test pg_disconnect_test` → `1 passed`.
    - [ ] **2.3** Terminal lifecycle and exit code 0 on both quit paths, panic hook restores terminal (`R-003`, `D-040`) — evidence: `cargo test -p pgtui --test cli_exit_test` → `2 passed`.
    - [ ] **2.4** `gallery` binary renders all ten screens via `pgtui::render` (`R-004`, `R-005`) — evidence: `cargo test -p pgtui --test gallery_test` → `3 passed`.
        - [ ] **2.4.1** `--out <dir>` creates 10 SVG + 10 PNG files with the D-071 names — evidence: `cargo test -p pgtui --test gallery_test writes_twenty_files` → `1 passed`.
        - [ ] **2.4.2** SVGs match protected `__svg` snapshots and are stable across runs — evidence: `cargo test -p pgtui --test gallery_test svg_matches_snapshots svg_deterministic` → `2 passed`.
    - [ ] **2.5** `docs/screens/` committed output and README screen list — evidence: `ls docs/screens | wc -l` → `20`; `grep -c 'docs/screens/' README.md` ≥ 10.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001`, `AC-003` — evidence: `cargo test -p pgtui --test app_disconnect_test` → `5 passed`.
    - [ ] **3.2** `AC-002`, `AC-004` — evidence: `cargo test -p pgtui --test pg_disconnect_test --test cli_exit_test` → `3 passed`.
    - [ ] **3.3** `AC-005`, `AC-006` — evidence: `INSTA_UPDATE=no cargo test -p pgtui --test gallery_test --test 'screen_*'` → `31 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only expected paths changed — evidence: `git status --porcelain`.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full output in the transcript.
```

- **verify.config:** `FOCUSED_CMDS` = five files; `REGRESSION_CMDS=("INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui" "cargo run -p pgtui --bin gallery -- --out target/gallery-verify")`; `REQUIRED_PATHS=("crates/pgtui/src/bin/gallery.rs" "docs/screens/screen__connection_list_empty.svg" "docs/screens/screen__custom_sql_results.png")`; `ALLOWED_GLOBS` += `"docs/screens/*" "README.md"`; `DENIED_GLOBS` += `"crates/pgtui/src/render.rs" "crates/pgtui/src/grid.rs" "crates/pgtui/src/store/*"`; forbidden += `'TODO\(TASK-106\)'`, `'process::exit\(|crates/pgtui/src/app.rs crates/pgtui/src/runtime.rs'` (exit only from `main.rs`).

Snapshot names (10, fixed): `screen__connection_list_empty`, `screen__connection_list_two`, `screen__create_form_blank`, `screen__create_form_filled`, `screen__browser_sidebar_empty_body`, `screen__preview_unsorted`, `screen__preview_sorted_asc`, `screen__preview_sorted_desc`, `screen__custom_sql_empty`, `screen__custom_sql_results`. (`screen__create_form_saved_list` in 102 is an 11th text-only snapshot; the gallery renders the ten above.) Each has a text `.snap` and a `__svg.snap`.

---

## 3. Fixture repo for TASK-101 (what exists at `baseline`)

```text
pgtui-fixture/                       git init; single commit; tag `baseline`
  Cargo.toml                         workspace; [workspace.dependencies] pinned (all six tasks' deps)
  Cargo.lock                         resolved by the planner (`cargo generate-lockfile` + one `cargo build`)
  rust-toolchain.toml                channel = "stable" (exact version pinned by the version agents)
  rustfmt.toml                       edition = "2024" only
  justfile                           test / test-unit / test-pg / lint / snap / gallery
  AGENTS.md                          real file (CLAUDE.md -> symlink to it): build/test/lint commands; "on start or resume read /progress/progress.md"; no architecture text (that lives in task.md)
  README.md                          one paragraph; TASK-106 appends the screen list
  .gitignore                         target/, *.snap.new
  crates/pgtui/Cargo.toml            [lib] + [[bin]] pgtui + [[bin]] gallery (path src/bin/gallery.rs, `required-features = []`, file absent until 106 → planner ships a 3-line stub that prints usage and exits 2 so the workspace builds)
  crates/pgtui/src/lib.rs            `pub mod render;` only  (other `pub mod` lines added by executors; lib.rs is in expected_paths for 101/103/104)
  crates/pgtui/src/main.rs           stub: `fn main() { eprintln!("error: not implemented"); std::process::exit(2) }`
  crates/pgtui/src/render.rs         PLANNER: Buffer→text, Buffer→SVG, SVG→PNG (resvg + embedded font via include_bytes). Protected from 101 on. Used by tests/support and by bin/gallery in 106.
  crates/pgtui/src/bin/gallery.rs    stub (see above); replaced in 106
  crates/pgtui/tests/support/mod.rs  PLANNER (D-070); thin wrappers over pgtui::render + key constructors + temp_store + pg_container
  crates/pgtui/tests/support/fake_data.rs
  crates/pgtui/tests/support/fonts/DejaVuSansMono.ttf   (also include_bytes'd from src/render.rs; one copy under src/fonts/, protected)
  crates/pgtui/tests/fixtures/seed.sql
  crates/pgtui/tests/snapshots/      ALL .snap files for tasks 101–106 (see §4 for why all, not just 101's)
  crates/pgtui/tests/*_test.rs       ALL trusted test files for tasks 101–106
```

Why ship every task's trusted tests and snapshots at 101 rather than adding them per task:

- Each `tests/*_test.rs` is its own crate; a file whose imports do not exist yet fails to compile **only when selected** (`--test <file>`). The full regression command `cargo test -p pgtui` would fail at 101 because it compiles every test target — so REGRESSION_CMDS for tasks 101–105 use an explicit list of the test files that belong to tasks ≤ N (`cargo test -p pgtui --test a --test b …`) instead of the bare crate. TASK-106 uses the bare crate.
- Advantage: the protected manifest for every task hashes the identical `tests/` tree; the executor sees future tests but cannot run them meaningfully and gains nothing from reading them (they reference the D-* contracts already in task.md).
- Alternative (rejected): add tests per task in the reference solution. That makes the reference solution both "code" and "trusted tests", blurring what the protected manifest proves and making the 106 completeness check depend on executor-visible history.

Trusted vs executor-written tests per task (summary):

| Task | Planner-shipped, protected (`tests/`) | Executor-written, in `src/` `#[cfg(test)]` | Enforced by |
| --- | --- | --- | --- |
| 101 | `store_test`, `screen_connection_list_test`, `app_connection_list_test`, `cli_test` | `keys::tests`, `store::tests` (path precedence) | leaf 2.5 count check |
| 102 | `app_create_form_test`, `runtime_create_test`, `screen_create_form_test` | `app::tests::validate_*` | leaf 2.5 |
| 103 | `pg_connect_test`, `app_browser_test`, `pg_runtime_connect_test`, `screen_browser_test` | `db::tests::quote_ident` | leaf 2.5 |
| 104 | `grid_sort_test`, `pg_preview_test`, `app_preview_test`, `screen_preview_test` | `grid::tests` comparator | leaf 2.1.3 |
| 105 | `app_custom_sql_test`, `pg_custom_sql_test`, `screen_custom_sql_test` | `db::tests::strip_trailing_semicolon` | leaf 2.5 |
| 106 | `app_disconnect_test`, `pg_disconnect_test`, `cli_exit_test`, `gallery_test` | none required | — |

Executor-written unit tests are not gate oracles (the executor could write vacuous ones); they exist to measure `instruction_violations`/scope behaviour and to keep the "tests directly proving it" scope line honest. The oracle is always the protected file.

Reference solutions: the planner builds the full app once (`ref/106`), then produces `ref/101..105` by checking out the task's expected_paths subset from that tree, verifying each gate FAILs on its baseline and PASSes on its reference (README step 5). The `baseline` tag of TASK-N+1 is the `ref/N` tree committed on top of the fixture.

---

## 4. Protecting snapshots the executor must "create"

Question: text/SVG snapshots are the AC for every screen, but insta's normal flow has the implementer write them (`cargo insta review`). How can they be protected?

Option A — **planner ships the expected `.snap` files as protected fixtures.**

- Pro: the gate is the oracle (D13: FAIL on baseline because the executor's render differs or the test does not compile; PASS with reference). Deterministic; harness-reproducible; no LLM judgement in the loop; `leaf_claim_accuracy` measurable.
- Pro: D13 already forces a reference solution, so the snapshots are a free by-product of a step that must happen anyway.
- Con: over-constrains layout — the executor must reproduce the exact frame. Mitigation: D-060 specifies layout to the character (titles, widths, markers, help strings). This is a feature for the experiment, not a bug: whether an executor reproduces a fully specified layout from a written contract is exactly a predictability measurement.
- Con: an executor may try `INSTA_UPDATE=always`/`cargo insta accept`. Mitigation: protected hashes on `.snap`, `*.snap.new` extra check, `INSTA_UPDATE=no INSTA_FORCE_PASS=0` on every gate command, forbidden pattern on `INSTA_` in `src/`.

Option B — **AC says "snapshots exist and are accepted", Layer-3 reviewer judges them.**

- Pro: no layout over-specification; closer to real-world practice.
- Con: violates "one canonical gate" — the verdict becomes a fresh-LLM judgement (§7 item 7 is itself an experiment variable; it cannot also be the oracle). Anthropic's own guidance: reviewers "will usually report some gaps even when the work is sound". Non-deterministic across seeds → poisons `gate_pass` and `false_done`.
- Con: an executor-created snapshot passes trivially (`assert_snapshot!` of whatever renders). The gate cannot distinguish a right screen from a wrong one.

**Pick: A**, with one softening: the PNG is never byte-compared (font rasterisation differs across platforms); the SVG is the canonical visual artifact and is text-snapshotted; PNG existence and pixel dimensions are asserted. The reviewer pass (§7.7), when run, gets `docs/screens/*.png` as an extra input for a *secondary* judgement that is recorded as a metric, never as the gate.

Consequence for authoring: D-060 must be complete before any reference solution is written, and any change to D-060 after dispatch of TASK-101 is a re-plan of every later task (their snapshots move). This is the cost of A and it is acceptable for a fixture.

---

## 5. Sequencing notes and open risks

1. **Baseline chain.** `baseline(102) = ref(101)`, …, `baseline(106) = ref(105)`. Each `ref/N` is produced from the single full implementation by path subset (§3), then gate-proven both ways. Because protected `tests/` are identical across tasks, the manifest differs only in which `src/` files join the protected set (store from 102, grid from 105, render always).
2. **Regression command scoping.** Tasks 101–105 list their test targets explicitly (future test files do not compile). The planner keeps one table `task → test files` and generates `REGRESSION_CMDS` from it; a linter (§7.9) should check that every file under `tests/` appears in exactly the tasks from its introduction onward.
3. **Docker inside the agent container** (tasks 103–106). testcontainers needs a Docker API. The headed-herdr harness note does not cover this; options are socket mount (`-v /var/run/docker.sock`) or DinD. UNVERIFIED which works with the non-root `agent` user (socket group). Until settled, 103–106 are dispatchable only on a host runner; P-004 (`docker info`) turns the gap into a clean `BLOCKED`, which is itself a `false_blocked` control (§6).
4. **Heap-order preview.** D-025 keeps `SELECT * … LIMIT 500` without `ORDER BY` per spec. Snapshots use `fake_data` (deterministic), and `pg_preview_test::preview_matches_fake` compares real rows to fake rows **as multisets** (sorted by primary key inside the test) so a heap reorder cannot flake the PG test; only the snapshot path relies on order, and it never touches PG.
5. **Simple-query and `tokio-postgres` version.** If the version agents pick `sqlx`, D-023/D-025 need rewriting (sqlx has no all-text simple-query surface exposing per-cell `Option<&str>` uniformly). Decide before writing the reference solution; do not let it leak into an executor's choice.
6. **Turso API stability.** `turso` is pre-1.0; the store API in D-022 is ours, so the executor is insulated, but the planner's reference solution pins the exact crate version in `Cargo.lock`. Executors cannot bump it (denied glob).
7. **Token budget.** Each `task.md` above carries 8–12 D-* items verbatim plus a 14–17-leaf checklist; estimated 1,900–2,400 tokens. TASK-104 is the fattest (D-050..D-053 + D-060 grid section); if it exceeds ~2,500, move the grid-widget rendering rules into a `docs/decisions/D-060.md` in the fixture and reference it in "Read before editing" (the pattern the example task uses for `D-041.md`).
8. **Experiment coverage.** The six tasks give `kind: feature` only. For the §6 minimum design (bugfix, feature, removal) the planner should later derive: a bugfix task from an injected defect in `ref/106` (e.g., NULLs sorted first in asc), and a removal task (delete the `x` custom-SQL screen and all its code, gate = forbidden patterns + snapshot set shrinks). Both reuse this fixture and its protected tests with small snapshot deltas.
