# TASK-001 — fixed decisions (verbatim)

Planner-owned and self-contained: every D-* in force for this task is written here in full. Numbering is global across the pgtui series; a D-id means the same thing in every task. Implement; do not reopen. Anything not covered here that changes public behaviour, architecture, data or security posture is `NEEDS_REPLAN`, not executor discretion.

In force for TASK-001: D-001..D-005 (workspace, pins, layout, lint, no task runner), D-040, D-070, D-071.

## Repository, workspace, exact pins

- **D-001 Workspace and exact pins.** Cargo workspace, `edition = "2024"`, `resolver = "3"`, single member `crates/pgtui` (lib + bins `pgtui`, `gallery`). The workspace `Cargo.toml` owns `[workspace.dependencies]` and `[workspace.lints.rust] unsafe_code = "forbid"`; the member manifest uses `workspace = true` for every runtime dependency. Executors never add, remove, or bump a dependency. Exact pins, all with `=`:
  runtime: `ratatui 0.30.2`, `crossterm 0.29.0`, `turso 0.7.2` (`default-features = false`), `tokio 1.53.1` (features `rt`, `macros`, `time`, `sync`), `tokio-postgres 0.7.18`, `clap 4.6.6` (feature `derive`), `thiserror 2.0.20`, `directories 6.0.0`, `resvg 0.48.1` (used only by the planner-shipped `render.rs`);
  dev: `tempfile 3.27.0`, `testcontainers 0.27.3`, `testcontainers-modules 0.15.0` (feature `postgres`), `nix 0.30.1` (feature `term`).
  `Cargo.lock` is committed and stays in sync. There is no `insta` and no `assert_cmd`: verification material asserts behaviour, never snapshots (D-071). `crates/pgtui/src/render.rs`, `crates/pgtui/src/fonts/` and everything under `crates/pgtui/tests/` are placed on the run base commit by the harness (trusted overlay, D-070); they are not created by this task, are outside `verify.toml`'s writable paths (D-003), and the manifests must still compile them.

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

  `render.rs` and `fonts/` are never created, edited, or deleted by the executor; they ship with the trusted overlay and are excluded from `verify.toml`'s writable paths.

- **D-003 Test placement and trusted material.** Trusted tests (planner-shipped) live in `crates/pgtui/tests/*_test.rs` with `crates/pgtui/tests/support/mod.rs` and `crates/pgtui/tests/fixtures/seed.sql`; they are protected, are outside `verify.toml`'s writable paths, and must not be created, edited, or deleted. Executor-written tests are `#[cfg(test)] mod tests` inside the `src/` module they test; no executor-written file goes under `tests/`.

- **D-004 Toolchain and lint gate.** `rust-toolchain.toml` pins `1.98.0`; `rustfmt.toml` sets `edition = "2024"`; `.cargo/config.toml` sets `TESTCONTAINERS_COMMAND = "remove"`; `.gitignore` lists `target/`. Gate lint: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`. Forbidden in `crates/pgtui/src/`: `#![allow(...)]`, `#[allow(clippy::...)]`, `#[allow(dead_code)]`, `#[allow(unused)]`, `todo!()`, `unimplemented!()`, `dbg!()`. Forbidden in `crates/pgtui/tests/`: `#[ignore]`.

- **D-005 No repository task runner.** No `justfile`, `Makefile`, `xtask`, or similar is created; executors call `cargo` and `taskfmt` directly. The repository `README.md` (with its `## Screens` section) is written by TASK-007 only; no earlier task creates it.

## Exit codes and errors

- **D-040 Exit codes.** `0` normal quit (`q`, `Ctrl+C`). `2` usage/config: bad CLI args, store open failure, cannot create data dir, unwritable output directory — message on stderr as `error: <msg>`, before entering raw mode. `1` unexpected runtime failure (terminal IO error, panic) — terminal restored by a panic hook first. From TASK-001 until its task replaces it, both binaries are stubs that print `error: not implemented` (`gallery`: usage) on stderr and exit 2.

## Trusted verification material (planner-shipped, protected)

- **D-070 `tests/support/mod.rs`.** Imported by every trusted test as `mod support;`. Provides the 100x30 render helpers `render_app(&App) -> String` (`ui::draw` into a fresh buffer, then `pgtui::render::buffer_to_text`), `render_app_svg(&App) -> String`, `render_app_png_dims(&App) -> (u32, u32)`, key constructors `key(char)`, `key_code(KeyCode)`, `ctrl(char)`, `enter()`, `tab()`, `back_tab()`, `backspace()`, `esc()`, `down()`, `up()`, `left()`, `right()`, `temp_store() -> (TempDir, ConnectionStore)`, `new_connection(name)`, `saved_connection(name)`, `unwritable_db_path()`; from TASK-004 also `fake_data` and `pg_container() -> (ContainerAsync<Postgres>, ConnParams)` starting `postgres:16-alpine` with `fixtures/seed.sql` applied via `with_init_sql`. The helper set grows per task; earlier tasks' tests keep compiling unchanged.

- **D-071 Behavioural verification, not golden snapshots.** Trusted tests assert behaviour only: substring presence, line ordering and counts on rendered text, store roundtrips, exit codes, and PNG dimensions read from the IHDR chunk. There are no insta snapshots and no golden bytes: any correct implementation passes, and the material stays valid when formatting or styling changes that do not alter behaviour. Every trusted test fails (compile error or assertion) while the feature it covers is absent. No `*.snap` files exist anywhere in the repository.
