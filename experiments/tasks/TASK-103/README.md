---
schema: task/v3
id: TASK-103
title: "Connect to PostgreSQL from a saved connection and list its tables in a sidebar"
kind: feature
verify: "/task/verify.sh"
expected_paths:
  - "crates/pgtui/src/db/*"
  - "crates/pgtui/src/app.rs"
  - "crates/pgtui/src/keys.rs"
  - "crates/pgtui/src/runtime.rs"
  - "crates/pgtui/src/ui/browser.rs"
  - "crates/pgtui/src/ui/mod.rs"
  - "crates/pgtui/src/lib.rs"
protected_paths:
  - "crates/pgtui/tests/*"
  - "crates/pgtui/src/render.rs"
  - "crates/pgtui/src/fonts/*"
  - "crates/pgtui/src/store/*"
  - "Cargo.toml"
  - "Cargo.lock"
  - "crates/pgtui/Cargo.toml"
  - "justfile"
  - "rust-toolchain.toml"
  - "AGENTS.md"
---

# TASK-103 — Connect to PostgreSQL from a saved connection and list its tables in a sidebar

Execution protocol, progress file grammar, and final report format are in `/task/AGENTS.md`. This file is read-only. It defines WHAT must become true; it does not change during execution.

## Goal

`Enter` on a saved connection opens the Browser screen with every user table in the left sidebar and an empty main body; a connection failure is shown as an error on the list screen.

## Context

Current behavior:

- `pgtui` (TASK-102 result) lists and creates connections; `Enter` on the list is a no-op, there is no `db` module, `Screen::Browser` is never entered, `ui/browser.rs` does not exist.
- `crates/pgtui/tests/pg_connect_test.rs` does not compile (`pgtui::db::postgres` unresolved).

Desired behavior:

- `Enter` → `Effect::Connect(selected)`; the runtime connects with `tokio-postgres` (`NoTls`, 5 s timeout), runs the D-025 tables query via `simple_query`, replies `Msg::Connected(Ok(tables))`; the app enters `Screen::Browser` with `SessionView { name, tables, sidebar_cursor: 0, focus: Sidebar }` and renders sidebar + empty main pane. `Msg::Connected(Err)` → `Status::Error`, screen stays `ConnectionList`.

Read before editing (non-normative hints, in order):

1. `AGENTS.md` — build/test/lint commands; `docker` is required for `pg_*` tests.
2. `/task/decisions.md` — verbatim D-* text (client/protocol D-023, DSN D-024, SQL D-025, types D-026, Browser keys D-033, layout D-060, seed D-072). Decided; do not reopen.
3. `crates/pgtui/tests/support/mod.rs` — `pg_container() -> (ContainerAsync<Postgres>, ConnParams)` and `fake_data::tables()`; shows the `ConnParams` and `PgSession` surface the tests compile against.
4. `crates/pgtui/tests/pg_connect_test.rs`, `app_browser_test.rs`, `pg_runtime_connect_test.rs`, `screen_browser_test.rs` — the oracles.
5. `crates/pgtui/src/app.rs`, `runtime.rs`, `keys.rs` — existing update/execute/key dispatch to extend.

Code flow: `db/mod.rs` defines `ConnParams { host, port, dbname, user, password }` (built from `SavedConnection` by `ConnParams::from(&SavedConnection)`), `TableRef`, `ResultSet`, `Cell`, `DbError { Connect(String), Query(String), Timeout, MultiStatement }`, and `quote_ident`. `db/postgres.rs` holds `PgSession { client, conn_task }`; `PgSession::connect(&ConnParams)` builds a `tokio_postgres::Config`, wraps `connect(NoTls)` in `tokio::time::timeout(5 s)`, spawns the connection future; `list_tables()` runs the D-025 query with `client.simple_query` and maps `SimpleQueryMessage::Row` to `TableRef`. `runtime::execute(Connect(saved))` calls both, stores the session in `Runtime.session`, replies `Connected`. `keys.rs` adds the Browser sidebar keys; `Enter` in the sidebar and `d` are no-ops in this task.

Baseline (run from repo root, before any edit):

```sh
cargo test -p pgtui --test pg_connect_test
```

Expected before this change: `error[E0433]`/`E0432` — `pgtui::db::postgres` unresolved.

## Preconditions

Each precondition has a command. If a command fails, stop and report `BLOCKED` (see AGENTS.md). Do not work around it.

- **P-001:** toolchain available — `cargo --version`
- **P-002:** test support present — `test -f crates/pgtui/tests/support/mod.rs`
- **P-003:** baseline tag resolvable — `git rev-parse --verify baseline`
- **P-004:** Docker reachable for testcontainers — `docker info >/dev/null`

## Scope

In scope:

- `db/mod.rs` types + `quote_ident`; `db/postgres.rs` `PgSession::connect` and `list_tables`; `pub mod db` in `lib.rs`.
- `Effect::Connect` execution with timeout; `Msg::Connected` handling; `SessionView`.
- Browser sidebar keys (`j`/`k`/`Up`/`Down` clamp; `Tab` no-op while `grid.is_none()`); `ui/browser.rs` sidebar + empty main; dispatch in `ui/mod.rs`.
- Executor unit test for `quote_ident` in `src/db/mod.rs`.

Out of scope:

- Previewing a table (`Enter` in the sidebar is a no-op), sorting, custom SQL (`x` no-op), disconnect (`d` no-op; `Ctrl+C` still quits), TLS/sslmode, connection pooling.
- Any change to `store/`, dependency changes, formatting sweeps.

## Requirements

- **R-001 (MUST):** `PgSession::connect(&ConnParams) -> Result<PgSession, DbError>` per D-023/D-024: key/value `Config`, `NoTls`, 5 s timeout → `DbError::Timeout`, refusal → `DbError::Connect`. `list_tables(&self) -> Result<Vec<TableRef>, DbError>` runs the D-025 query via `simple_query` and returns rows in server order.
- **R-002 (MUST):** `Msg::Connected(Ok(tables))` → `Screen::Browser`, `session = Some(SessionView { name: <connection name>, tables, sidebar_cursor: 0, focus: Focus::Sidebar })`, `grid = None`.
- **R-003 (MUST):** `Msg::Connected(Err(e))` → stay on `ConnectionList`, `Status::Error(<first line of e>)`, `session` unchanged.
- **R-004 (MUST):** Sidebar navigation clamps per D-033; `Enter` on an empty list emits nothing.
- **R-005 (MUST):** Render per D-060 Browser: `[Length(30)] + [Min(0)]`, sidebar title ` Tables (<n>)* ` when focused, main title ` <connection name> ` with empty body, help text.
- **R-006 (MUST NOT):** Mark any `pg_*` test `#[ignore]`, skip it when Docker is absent, or use prepared-statement APIs (`query`, `query_one`, `prepare`, `execute`) in `db/` — simple-query only (D-023).
- **R-007 (MUST):** Implement the final design directly; no dependency changes; no edits to `Cargo.toml`, `Cargo.lock`, `justfile`, `rust-toolchain.toml`, `store/`, `render.rs`, or `tests/`. Executor tests live in `src/` only (D-003).

## Acceptance criteria

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given a seeded container, when `connect` then `list_tables`, then the four seed tables in D-072 order. | `cargo test -p pgtui --test pg_connect_test lists_seed_tables` | exit 0, `1 passed` |
| AC-002 | Given a closed port on `127.0.0.1`, when `connect`, then `Err(DbError::Connect)` or `Err(DbError::Timeout)` within 6 s. | `cargo test -p pgtui --test pg_connect_test refused_port_errors_fast` | exit 0, `1 passed` |
| AC-003 | Given the list with a connection, when `Enter` then `Msg::Connected(Ok(tables))`, then Browser with cursor 0 and Sidebar focus; `Msg::Connected(Err)` keeps the list with `error:`; sidebar `j`/`k` clamp. | `cargo test -p pgtui --test app_browser_test` | exit 0, `5 passed` |
| AC-004 | Given the runtime with the container's params, when `Effect::Connect` executes, then the reply is `Connected(Ok)` with the seed tables. | `cargo test -p pgtui --test pg_runtime_connect_test` | exit 0, `1 passed` |
| AC-005 | Given four fake tables, when rendered with the cursor on `public.customers`, then `screen__browser_sidebar_empty_body` (text + svg) matches. | `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_browser_test` | exit 0, `2 passed` |
| AC-006 | Given the real PG table list, when compared with `fake_data::tables()`, then equal. | `cargo test -p pgtui --test pg_connect_test fake_tables_match_pg` | exit 0, `1 passed` |

## Fixed decisions

Already decided. Implement; do not reopen. Verbatim text in `/task/decisions.md`. Anything not listed there that changes public behavior, architecture, data, or security posture is `NEEDS_REPLAN`.

- **D-010..D-013:** `Screen`/`App`/`SessionView`/`Focus`, `Msg`/`Effect`, pure `update`, runtime loop, status line.
- **D-023..D-026:** `tokio-postgres` simple-query protocol, key/value DSN with `NoTls` and 5 s timeout, exact tables SQL, `ResultSet`/`Cell`/`TableRef`.
- **D-033:** Browser keys (sidebar subset active here).
- **D-041:** error surfacing.
- **D-060, D-061:** Browser layout strings, `ui::draw`.
- **D-070..D-072:** `pg_container`, `fake_data`, snapshot policy, seed fixture and its table order.

## Checklist

Static plan. IDs `N`…`N.N.N.N`, max depth 4, four spaces per level. State lives in `progress.md`.

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-004` pass — evidence: each command exits 0 (incl. `docker info`).
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test pg_connect_test` fails to compile (`pgtui::db::postgres` missing).
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** PostgreSQL session (`R-001`, `R-006`, `D-023..D-025`) — evidence: `cargo test -p pgtui --test pg_connect_test` → `3 passed`.
        - [ ] **2.1.1** `PgSession::connect` builds the D-024 config, uses `NoTls`, times out at 5 s — evidence: `cargo test -p pgtui --test pg_connect_test refused_port_errors_fast` → `1 passed`.
        - [ ] **2.1.2** `list_tables` runs the D-025 query via `simple_query` in server order — evidence: `cargo test -p pgtui --test pg_connect_test -- lists_seed_tables fake_tables_match_pg` → `2 passed`.
    - [ ] **2.2** App handles `Connected` and sidebar keys (`R-002..R-004`, `D-033`) — evidence: `cargo test -p pgtui --test app_browser_test` → `5 passed`.
        - [ ] **2.2.1** `Enter` on the list emits `Effect::Connect(selected)`; `Connected(Ok)` enters Browser with cursor 0, Sidebar focus — evidence: `cargo test -p pgtui --test app_browser_test enter_connect` → `2 passed`.
        - [ ] **2.2.2** `Connected(Err)` stays on the list with `Status::Error`; `j`/`k` clamp in the sidebar — evidence: `cargo test -p pgtui --test app_browser_test -- connect_error sidebar_` → `3 passed`.
    - [ ] **2.3** `runtime::execute(Connect)` connects, lists, stores the session, replies (`R-001`, `D-012`) — evidence: `cargo test -p pgtui --test pg_runtime_connect_test` → `1 passed`.
    - [ ] **2.4** Browser renders sidebar + empty body + focus marker per `D-060` (`R-005`) — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_browser_test` → `2 passed`.
    - [ ] **2.5** Unit test for `quote_ident` exists in `src/db/mod.rs` — evidence: `cargo test -p pgtui --lib db::tests::quote_ident` → `≥ 1 passed`.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001`, `AC-002`, `AC-006` — evidence: `cargo test -p pgtui --test pg_connect_test` → `3 passed`.
    - [ ] **3.2** `AC-003`, `AC-004` — evidence: `cargo test -p pgtui --test app_browser_test --test pg_runtime_connect_test` → `6 passed` across both binaries.
    - [ ] **3.3** `AC-005` — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_browser_test` → `2 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated — evidence: `git status --porcelain` shows only those paths.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full verifier output shown in the transcript.
<!-- checklist:end -->
