# Fixtures, seed and trusted material

Two things used to live here that no longer do: per-task fixture repositories (`pgtui-101..103`) and planner-authored application code. The target repository now starts **empty** (`main` = one allow-empty bootstrap commit) and every artifact is written by a task; the verification material that used to be fixture content is shipped **per task** in `tasks/<TASK-ID>/trusted/`. This file specifies that material. It is a spec, not the code.

## `fixtures/seed/`

Kept unchanged: the default seed dir mounted `/seed:ro` in every run (`pgtui-seed.sql` — the same D-072 rows as `tests/fixtures/seed.sql`, for the standing `prereq-postgres`).

## Trusted overlay model

For each run the harness clones the target repository, copies the task's `trusted/` tree over it, and commits the result as the **run base commit** (pushed to `main` ahead of the agent's commit, D25). The gate then scopes and verifies against that base:

- `trusted/crates/pgtui/src/render.rs` and `trusted/crates/pgtui/src/fonts/DejaVuSansMono.ttf` — planner-owned renderer (Buffer → text/SVG/PNG via `resvg` 0.48.1 with the bundled font). Exposes `buffer_to_text`, `buffer_to_svg`, `svg_to_png` (+ the `png_dimensions` IHDR helper). Present in **every** task, never in `expected_paths`/`allowed_globs`, listed in `forbidden_paths`.
- `trusted/crates/pgtui/tests/` — cumulative trusted tests for tasks 1..N, plus `tests/support/mod.rs` (and from TASK-004 `tests/support/fake_data.rs`, `tests/fixtures/seed.sql`). Read-only for the executor.
- Every trusted test must **fail** (compile error or assertion) on its task's base commit and **pass** for any correct implementation; the set is monotone — later tasks never invalidate earlier assertions.
- Assertions are behavioural only: substring/order/count on rendered text, store roundtrips, exit codes, PNG dimensions from the IHDR chunk. No insta snapshots, no golden bytes, no `.snap` files (D-071).
- `support` helpers grow per task; earlier tests keep compiling. `render_app(&App)` draws with `ui::draw(app, &mut Buffer)` at 100x30 and pipes through `buffer_to_text`.

## Pinned dependencies (`[workspace.dependencies]`, D-001)

Versions verified 2026-08-28 in a scratch workspace that compiles all 23 trusted test files.

| Crate | Version | Features / notes |
| --- | --- | --- |
| `ratatui` | =0.30.2 | default (crossterm 0.29 backend) |
| `crossterm` | =0.29.0 | `Msg::Key(KeyEvent)` types |
| `turso` | =0.7.2 | `default-features = false` |
| `tokio` | =1.53.1 | `rt`, `macros`, `time`, `sync` |
| `tokio-postgres` | =0.7.18 | `NoTls`; simple-query protocol only (D-023). Not sqlx. |
| `clap` | =4.6.6 | `derive` |
| `thiserror` | =2.0.20 | |
| `directories` | =6.0.0 | XDG data dir for D-020 |
| `resvg` | =0.48.1 | normal dep; `usvg`/`tiny-skia` via re-export |
| dev `tempfile` | =3.27.0 | `temp_store`, gallery output dirs |
| dev `testcontainers` | =0.27.3 | `ImageExt`/`AsyncRunner` (`with_init_sql`, `with_tag`) |
| dev `testcontainers-modules` | =0.15.0 | `postgres`; image `postgres:16-alpine` |
| dev `nix` | =0.30.1 | feature `term` only (`openpty` for `cli_exit_test`) |

No `insta`, no `assert_cmd` (D-071). Edition `2024`, `rust-version = "1.88"`, `rust-toolchain.toml` pins `1.98.0`. The 0.28.0 `testcontainers` pin in the old fixture spec is unresolvable with modules 0.15.0 — use the pair above.

## Trusted test inventory (task → files → counts)

Counts are the assertions each gate runs; they are the numbers quoted in the READMEs.

| Task | File (tests) |
| --- | --- |
| 001 | `skeleton_test.rs` (4): `pgtui_stub_exits_2`, `gallery_stub_exits_2`, `buffer_to_text_trims_and_keeps_rows`, `svg_and_png_pipeline_is_deterministic` (900x540) |
| 002 | `store_test.rs` (5), `app_connection_list_test.rs` (5), `screen_connection_list_test.rs` (3), `cli_test.rs` (3) |
| 003 | `app_create_form_test.rs` (9), `runtime_create_test.rs` (3), `screen_create_form_test.rs` (3) |
| 004 | `pg_connect_test.rs` (4), `pg_runtime_connect_test.rs` (1), `app_browser_test.rs` (6), `screen_browser_test.rs` (2) |
| 005 | `grid_sort_test.rs` (8), `pg_preview_test.rs` (4), `app_preview_test.rs` (8), `screen_preview_test.rs` (5) |
| 006 | `app_custom_sql_test.rs` (9), `pg_custom_sql_test.rs` (5), `screen_custom_sql_test.rs` (3) |
| 007 | `app_disconnect_test.rs` (5), `pg_disconnect_test.rs` (1), `cli_exit_test.rs` (2), `gallery_test.rs` (4) |

Total 102 across 23 targets; the per-task cumulative regression command in each `verify.toml` runs all targets up to that task.

## Seed fixture (`tests/fixtures/seed.sql`, D-072)

`DROP`-first preamble, then schema `public`: `customers(id int PK, name text, balance numeric(10,2), signup_date date, note text NULL)` 5 rows (two NULL notes; balances 10.50, 250.00, -3.25, 99.99, 250.00), `orders(id int PK, customer_id int, total numeric(10,2), status text)` 7 rows, `empty_table(id int PK)` 0 rows; schema `audit`: `events(id bigint PK, kind text, at timestamptz)` 3 rows. Sidebar order: `audit.events, public.customers, public.empty_table, public.orders`. `fake_data.rs` reproduces the rows cell-for-cell as text (`Cell::Text("10.50")`, `Cell::Null`, `2024-01-05`, `2024-06-01 10:00:00+00`); the empty string in a fixture row means SQL NULL.

## Readiness proof

For each task N: `taskfmt verify` must FAIL on the run base commit (trusted tests do not compile or fail while the feature is absent) and PASS for a correct implementation of that task's `expected_paths`. Scope protection is the whitelist (`allowed_globs` == `expected_paths`, D23/D28) plus `forbidden_paths` for the planner-owned renderer. PG-backed tests need Docker (`docker info`) and the image `postgres:16-alpine`.
