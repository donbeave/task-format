# Fixtures

Small repositories the task packages act on. Each fixture is a git repo with a single commit tagged `baseline`; the harness copies it to `/work` per run. Task dirs name their fixture in `experiments/tasks/<ID>/fixture`.

| Fixture | Baseline content | Used by |
| --- | --- | --- |
| `pgtui-101` | the skeleton described below | TASK-101 |
| `pgtui-102` | `pgtui-101` + reference solution of TASK-101 (`ref/101`) committed, re-tagged `baseline` | TASK-102 |
| `pgtui-103` | `pgtui-102` + `ref/102` | TASK-103 |

Only `pgtui-101` is authored by hand; the others are derived (decomposition note §3/§5). This file specifies what `pgtui-101` must contain. It is a spec, not the code.

## `pgtui-101` — required tree

```text
pgtui-101/
  Cargo.toml                         workspace, resolver = "3"; [workspace.dependencies] pinned (below)
  Cargo.lock                         resolved once by the planner (`cargo generate-lockfile` + `cargo build --all-targets`); protected in every task
  rust-toolchain.toml                channel = "1.98.0" (stable at authoring time; edition 2024 needs >= 1.85)
  rustfmt.toml                       edition = "2024"
  .gitignore                         target/  *.snap.new
  .cargo/config.toml                 [env] TESTCONTAINERS_COMMAND = "remove"
  justfile                           test, test-unit, test-pg, lint, snap, gallery (D-005)
  CLAUDE.md                          agent-facing repo instructions (below); AGENTS.md -> symlink to CLAUDE.md
  README.md                          one paragraph: what pgtui is; TASK-106 appends the screen list
  crates/pgtui/Cargo.toml            [lib] + [[bin]] pgtui (src/main.rs) + [[bin]] gallery (src/bin/gallery.rs); all deps `workspace = true`
  crates/pgtui/src/lib.rs            exactly `pub mod render;`
  crates/pgtui/src/main.rs           stub: `fn main() { eprintln!("error: not implemented"); std::process::exit(2) }`
  crates/pgtui/src/bin/gallery.rs    stub: prints usage to stderr, exits 2 (replaced in TASK-106)
  crates/pgtui/src/render.rs         PLANNER, protected: Buffer -> text, Buffer -> SVG, SVG -> PNG (resvg, font via include_bytes!)
  crates/pgtui/src/fonts/DejaVuSansMono.ttf   bundled font (Bitstream Vera licence), the only font resvg may see
  crates/pgtui/tests/support/mod.rs          PLANNER, protected (D-070)
  crates/pgtui/tests/support/fake_data.rs    in-memory TableRefs/ResultSets identical to seed.sql (D-072)
  crates/pgtui/tests/fixtures/seed.sql       PostgreSQL seed applied by testcontainers `with_init_sql`
  crates/pgtui/tests/snapshots/*.snap        ALL text + `__svg` snapshots for TASK-101..106 (22 files)
  crates/pgtui/tests/*_test.rs               ALL trusted test files for TASK-101..106 (22 files)
```

Nothing under `crates/pgtui/src/` other than `lib.rs`, `main.rs`, `bin/gallery.rs`, `render.rs`, `fonts/` exists at baseline. Executors create `app.rs`, `keys.rs`, `store/`, `db/`, `grid.rs`, `runtime.rs`, `ui/` per D-002.

## Pinned dependencies (`[workspace.dependencies]`)

Every crate any of the six tasks needs is declared here so no task ever edits `Cargo.toml`. Versions verified 2026-08-28 (stack + testing notes).

| Crate | Version | Features / notes |
| --- | --- | --- |
| `ratatui` | 0.30.2 | default (crossterm 0.29 backend) |
| `crossterm` | 0.29.0 | |
| `turso` | 0.7.2 | `default-features = false` (no mimalloc, no fts) |
| `tokio` | 1.53.1 | `rt`, `macros`, `time`, `sync` |
| `tokio-postgres` | 0.7.18 | `NoTls`; simple-query protocol only (D-023). **Not** sqlx. |
| `clap` | 4.6.6 | `derive` |
| `thiserror` | 2.0.20 | |
| `directories` | 6.0.0 | XDG data dir for D-020 |
| `resvg` / `usvg` | 0.48.1 | used by `src/render.rs` (normal dep, not dev: `bin/gallery` needs it) |
| `tiny-skia` | 0.12 | via resvg re-export |
| dev `insta` | 1.48.0 | no extra features |
| dev `testcontainers` | 0.28.0 | |
| dev `testcontainers-modules` | 0.15.0 | `postgres`; image `postgres:16-alpine` (`with_tag`) |
| dev `tempfile` | latest 3.x at lock time | `temp_store`, `--db` tests |
| dev `assert_cmd` | latest 2.x at lock time | `cli_test` |

Edition `2024`, `rust-version = "1.88"` (ratatui 0.30 MSRV; sqlx's 1.94 floor no longer applies). `Cargo.lock` must build offline after one warm-up on the harness image.

## `CLAUDE.md` content (fixture, protected)

Commands only, no architecture (architecture lives in `README.md`/`decisions.md`):

- build: `cargo build --workspace --all-targets`
- unit: `cargo test -p pgtui --lib`
- one trusted file: `cargo test -p pgtui --test <name>`
- PG tests need Docker: `docker info` must succeed; image `postgres:16-alpine`
- lint: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings`
- snapshots: never run `cargo insta accept`/`INSTA_UPDATE=always`; `.snap` files are read-only
- "On start or resume read `/progress/progress.md` first."

## `justfile` recipes (D-005)

`test` (`cargo test -p pgtui`), `test-unit` (`--lib`), `test-pg` (`cargo test -p pgtui --test 'pg_*'`), `lint` (fmt + clippy as above), `snap` (`INSTA_UPDATE=no cargo insta test --check -p pgtui`), `gallery` (`cargo run -p pgtui --bin gallery -- --out docs/screens`).

## `seed.sql` (D-072)

```sql
CREATE SCHEMA audit;
CREATE TABLE public.customers (id int PRIMARY KEY, name text, balance numeric(10,2), signup_date date, note text);
INSERT INTO public.customers VALUES
  (1,'Ada',10.50,'2024-01-05','vip'), (2,'Bob',250.00,'2024-02-10',NULL),
  (3,'Cyd',-3.25,'2024-03-15','refund'), (4,'Dee',99.99,'2024-04-20',NULL), (5,'Eve',250.00,'2024-05-25','tie');
CREATE TABLE public.orders (id int PRIMARY KEY, customer_id int, total numeric(10,2), status text);
INSERT INTO public.orders VALUES (1,1,5.00,'paid'),(2,1,7.50,'paid'),(3,2,100.00,'open'),(4,3,1.25,'refunded'),
  (5,4,49.99,'paid'),(6,5,200.00,'open'),(7,5,50.00,'paid');
CREATE TABLE public.empty_table (id int PRIMARY KEY);
CREATE TABLE audit.events (id bigint PRIMARY KEY, kind text, at timestamptz);
INSERT INTO audit.events VALUES (1,'login','2024-06-01T10:00:00Z'),(2,'query','2024-06-01T10:05:00Z'),(3,'logout','2024-06-01T10:30:00Z');
```

`fake_data.rs` must reproduce these rows cell-for-cell as text (`Cell::Text("10.50")`, `Cell::Null`, dates as `2024-01-05`, timestamps as PostgreSQL renders them for the container's default timezone `UTC`: `2024-06-01 10:00:00+00`). Table order: `audit.events, public.customers, public.empty_table, public.orders`.

## `tests/support/mod.rs` (D-070)

```rust
pub fn render_text(app: &App) -> String;          // TestBackend 100x30 -> lines, trailing spaces trimmed
pub fn render_svg(app: &App) -> String;           // pgtui::render::buffer_to_svg, deterministic
pub fn svg_to_png(svg: &str) -> Vec<u8>;          // pgtui::render::svg_to_png; tests assert len > 0 and 900x540 px
pub fn key(c: char) -> Msg;  pub fn key_code(k: KeyCode) -> Msg;  pub fn ctrl(c: char) -> Msg;
pub async fn temp_store() -> (tempfile::TempDir, ConnectionStore);
pub async fn pg_container() -> (ContainerAsync<Postgres>, ConnParams);   // postgres:16-alpine + seed.sql
pub mod fake_data { pub fn tables() -> Vec<TableRef>; pub fn preview(t: &TableRef) -> ResultSet; }
```

Each test file declares `mod support;` and only uses the items above plus the public `pgtui::*` API fixed in decisions. `pg_container` keeps the container alive for the test's whole scope (containers are removed on `Drop`; no Ryuk in testcontainers-rs).

## Trusted test files and expected counts

Test names are load-bearing: task checklists filter on them.

| File | Task | Tests |
| --- | --- | --- |
| `store_test.rs` | 101 | `open_creates_schema`, `insert_then_list_sorted_by_name`, `reopen_persists`, `display_dsn_hides_password`, `duplicate_name_rejected` (5) |
| `app_connection_list_test.rs` | 101 | `connections_msg_populates_list`, `j_clamps_at_end`, `k_clamps_at_start`, `q_emits_quit`, `ctrl_c_emits_quit` (5) |
| `screen_connection_list_test.rs` | 101 | `screen__connection_list_empty`, `..._empty_svg`, `screen__connection_list_two`, `..._two_svg` (4) |
| `cli_test.rs` | 101 | `unwritable_db_exits_2`, `version_exits_0`, `help_exits_0` (3; `assert_cmd`, no PTY) |
| `app_create_form_test.rs` | 102 | `editing_n_opens_blank_form`, `editing_tab_wraps`, `editing_chars_and_backspace`, `editing_port_digits_only`, `validation_empty_name`, `validation_empty_host`, `validation_port_zero` (7) |
| `runtime_create_test.rs` | 102 | `save_roundtrip`, `duplicate_keeps_form`, `esc_discards` (3) |
| `screen_create_form_test.rs` | 102 | `screen__create_form_blank`, `screen__create_form_filled`, `screen__create_form_saved_list` × (text, svg) (6) |
| `pg_connect_test.rs` | 103 | `lists_seed_tables`, `refused_port_errors_fast`, `fake_tables_match_pg` (3) |
| `app_browser_test.rs` | 103 | `enter_connect_emits_effect`, `enter_connect_ok_enters_browser`, `connect_error_stays_on_list`, `sidebar_j_clamps`, `sidebar_k_clamps` (5) |
| `pg_runtime_connect_test.rs` | 103 | `runtime_connect_replies_seed_tables` (1) |
| `screen_browser_test.rs` | 103 | `screen__browser_sidebar_empty_body` × (text, svg) (2) |
| `grid_sort_test.rs`, `pg_preview_test.rs`, `app_preview_test.rs`, `screen_preview_test.rs` | 104 | per decomposition §2 |
| `app_custom_sql_test.rs`, `pg_custom_sql_test.rs`, `screen_custom_sql_test.rs` | 105 | per decomposition §2 |
| `app_disconnect_test.rs`, `pg_disconnect_test.rs`, `cli_exit_test.rs`, `gallery_test.rs` | 106 | per decomposition §2 |

Snapshot files: `tests/snapshots/<file>__<name>.snap` and `<file>__<name>__svg.snap` for every name in D-071 (insta default naming; `set_prepend_module_to_snapshot(false)` in support so the prefix is the test binary only). Snapshots are produced once from the planner's reference solution `ref/106` and never regenerated by an executor.

## Readiness proof (README step 5)

Before dispatch, for each task N in 101..103: `verify.sh` must FAIL on `pgtui-N` at `baseline` and PASS with `ref/N` applied. Record both outputs next to the fixture as `pgtui-N/.gate-proof/{baseline,ref}.log` (outside the tree the agent sees). `protected.sha256` is generated from the fixture root with `manifest.sh gen -o experiments/tasks/TASK-N/protected.sha256 <protected paths>`.
