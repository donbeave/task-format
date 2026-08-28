---
schema: task/v4
id: TASK-004
title: "Connect to PostgreSQL and list its tables"
kind: feature
verify: "taskfmt verify"
expected_paths:
  - "Cargo.toml"
  - "Cargo.lock"
  - "crates/pgtui/Cargo.toml"
  - "crates/pgtui/src/lib.rs"
  - "crates/pgtui/src/app.rs"
  - "crates/pgtui/src/keys.rs"
  - "crates/pgtui/src/runtime.rs"
  - "crates/pgtui/src/db/mod.rs"
  - "crates/pgtui/src/db/postgres.rs"
  - "crates/pgtui/src/ui/mod.rs"
  - "crates/pgtui/src/ui/browser.rs"
---

# TASK-004 — Connect to PostgreSQL and list its tables

Execution protocol, progress grammar, report format: `/task/AGENTS.md`. This file is read-only and states WHAT must become true.

## Goal

`Enter` on a saved connection opens a live PostgreSQL session over the simple-query protocol, lists its base tables in the browser sidebar, and failures degrade to a status error on the list.

## Context

Current behavior:

- TASK-003 completed the create form; `Enter` on the connection list is still a no-op, there is no `db/` module and no browser screen, and `pgtui` does not link `tokio-postgres` into `src/`.
- Trusted tests `pg_connect_test.rs`, `pg_runtime_connect_test.rs`, `app_browser_test.rs`, `screen_browser_test.rs` are committed on the run base commit and fail to compile; `tests/fixtures/seed.sql` and `tests/support/fake_data.rs` are already there.

Desired behavior:

- `db/mod.rs` with the D-026 types and `quote_ident`; `db/postgres.rs` with `PgSession::connect`/`list_tables` per D-023..D-025; `Effect::Connect` handled in `runtime.rs`; `SessionView` and the browser sidebar per D-010/D-033/D-060. Preview, `x`, and `d` stay inert (later tasks).

Read before editing (in order):

1. `/task/decisions.md` — D-023..D-026 (protocol, config, queries, types) and D-033/D-060 (browser) are the contract for this task.
2. `crates/pgtui/tests/pg_connect_test.rs` and `crates/pgtui/tests/pg_runtime_connect_test.rs` — the live-server oracles; they start `postgres:16-alpine` with `fixtures/seed.sql`.
3. `crates/pgtui/tests/app_browser_test.rs` and `crates/pgtui/tests/screen_browser_test.rs` — state and rendering oracles for the sidebar.

Code flow: `Enter` on a non-empty list emits `Effect::Connect(saved)`; `runtime::execute` builds `ConnParams::from(&saved)`, calls `PgSession::connect` (5 s timeout, D-024), then `list_tables` (D-025), and replies `Msg::Connected(..)`. `Ok` fills `session: Some(SessionView)` and switches to `Screen::Browser`; `Err` keeps the list and sets `Status::Error`. `db/postgres.rs` is the only place `tokio_postgres` is named; every statement goes through `simple_query`.

Baseline, from the repo root:

```sh
cargo test -p pgtui --test pg_connect_test
```

Expected before this change: a compile error — `crates/pgtui/src/db` does not exist.

## Preconditions

If a command fails, stop and report `BLOCKED`. Never work around a precondition. Docker is required from this task on.

- **P-001:** toolchain available — `cargo --version`
- **P-002:** TASK-003 gate green — `taskfmt verify`
- **P-003:** docker reachable — `docker info >/dev/null`
- **P-004:** trusted fixtures present — `test -f crates/pgtui/tests/fixtures/seed.sql && test -f crates/pgtui/tests/support/fake_data.rs`
- **P-005:** working tree clean — `test -z "$(git status --porcelain)"`

## Scope

In scope:

- `db/mod.rs`, `db/postgres.rs`, `PgSession`, `Effect::Connect` in `runtime.rs`, `SessionView`/`Focus` in `app.rs`, browser sidebar in `ui/browser.rs`, `lib.rs` module declarations.

Out of scope:

- `grid.rs`, `ui/grid.rs`, preview queries, `custom_sql.rs`; TASK-005/TASK-006.
- Disconnect (`d`), `x`, gallery, `README.md`.
- `render.rs`, `fonts/`, anything under `crates/pgtui/tests/`, the store contract.

## Requirements

- **R-001 (MUST):** Types exactly as D-026, including `PREVIEW_LIMIT = 500`, `Cell`, `ResultSet`, `TableRef` with `Display = "schema.name"`, `DbError` variants, `quote_ident`.
- **R-002 (MUST):** Every statement uses `Client::simple_query`; `query`, `query_one`, `query_raw`, `prepare`, `execute` never appear in `crates/pgtui/src/db/` (D-023).
- **R-003 (MUST):** Config per D-024: built from `ConnParams`, `application_name=pgtui`, `NoTls`, 5 s connect timeout mapping to `DbError::Timeout`; connect failure is non-fatal and leaves the list on screen.
- **R-004 (MUST):** Table listing uses the exact D-025 SQL; result order is `table_schema, table_name`.
- **R-005 (MUST):** Browser per D-033/D-060: `Tab` toggles focus only when a grid exists, sidebar `j`/`k`/`Up`/`Down` clamp, `Enter` on an empty sidebar is a no-op, sidebar block ` Tables (<n>) ` with `> ` marker and `*` on the focused pane, main block titled with the connection name while no grid is loaded.
- **R-006 (MUST):** Final design directly; no compatibility layer, dual path, deprecated alias, feature flag, or legacy fallback.
- **R-007 (MUST NOT):** Remove or bump a D-001 pin, or depend on anything outside the D-001 pin list (adding a listed pin is required where trusted tests need it); touch `render.rs`, `fonts/`, or anything under `crates/pgtui/tests/`; implement preview, custom SQL, sort, disconnect, or the gallery; create `grid.rs`, `custom_sql.rs`, `justfile`, or `README.md`.

## Acceptance criteria

Observable behaviour plus the exact evidence command. The gate runs these; the harness re-runs them.

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given a seeded `postgres:16-alpine` container, when `PgSession` connects and lists tables, then the four seed tables come back in schema order and refused ports/bad passwords map to the decided errors. | `cargo test -p pgtui --test pg_connect_test` | exit 0, `4 passed` |
| AC-002 | Given a live session, when `Effect::Connect` is executed, then it replies `Msg::Connected(Ok(tables))` with the same four tables. | `cargo test -p pgtui --test pg_runtime_connect_test` | exit 0, `1 passed` |
| AC-003 | Given the list screen, when connection effects and browser keys are applied, then `Connected(Ok)` enters the browser, `Connected(Err)` stays on the list, and sidebar cursors clamp. | `cargo test -p pgtui --test app_browser_test` | exit 0, `6 passed` |
| AC-004 | Given a connected `App`, when the 100x30 buffer is rendered, then the sidebar lists `schema.name` rows with the `> ` marker and focused-pane `*` as D-060 states. | `cargo test -p pgtui --test screen_browser_test` | exit 0, `2 passed` |
| AC-005 | Given the whole task, when all earlier trusted tests run, then they still pass. | `cargo test -p pgtui --test skeleton_test --test store_test --test app_connection_list_test --test screen_connection_list_test --test cli_test --test app_create_form_test --test runtime_create_test --test screen_create_form_test --test pg_connect_test --test app_browser_test --test pg_runtime_connect_test --test screen_browser_test` | exit 0, `48 passed` |
| AC-006 | Given the finished task, when the gate runs, then it reports `DONE`. | `taskfmt verify` | exit 0, last line `DONE` |

## Fixed decisions

Implement; do not reopen. Verbatim text in `/task/decisions.md`. Anything not decided there that changes public behaviour, architecture, data or security posture is `NEEDS_REPLAN`, not executor discretion.

- **D-023, D-024:** simple-query protocol only; config, timeout, non-fatal failure.
- **D-025:** exact table-listing SQL; `PREVIEW_LIMIT` declared now.
- **D-026:** `db/` type surface and `PgSession` API.
- **D-033, D-060:** browser keys and layout for this task's subset.
- **D-072:** seed fixture content the trusted containers apply.
- **D-070, D-071:** trusted helpers and behavioural verification.

## Checklist

Static plan. Hierarchical IDs, four spaces per level, max depth 4. Every leaf names what becomes true and the evidence permitting the check.

<!-- checklist:start -->
- [ ] **1** Baseline and environment are reproduced.
    - [ ] **1.1** Preconditions `P-001..P-005` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test pg_connect_test` fails to compile with `db` unresolved.
- [ ] **2** Database layer is implemented.
    - [ ] **2.1** D-026 types and `quote_ident` in `db/mod.rs` (`R-001`) — evidence: `grep -q 'pub const PREVIEW_LIMIT: usize = 500' crates/pgtui/src/db/mod.rs && grep -q 'pub fn quote_ident' crates/pgtui/src/db/mod.rs` exits 0.
    - [ ] **2.2** `PgSession::connect`/`list_tables` per D-023..D-025 (`R-002`, `R-003`, `R-004`, `AC-001`) — evidence: `cargo test -p pgtui --test pg_connect_test` prints `4 passed`.
    - [ ] **2.3** Only `simple_query` is used under `db/` (`R-002`) — evidence: `! grep -rnE 'query_one|query_raw|\.prepare\(' crates/pgtui/src/db` exits 0.
- [ ] **3** Connect flow is wired.
    - [ ] **3.1** `Effect::Connect` executes and replies `Msg::Connected` (`AC-002`) — evidence: `cargo test -p pgtui --test pg_runtime_connect_test` prints `1 passed`.
    - [ ] **3.2** Browser state transitions and sidebar cursors per D-033/D-010 (`AC-003`) — evidence: `cargo test -p pgtui --test app_browser_test` prints `6 passed`.
- [ ] **4** Rendering is implemented.
    - [ ] **4.1** `ui/browser.rs` sidebar per D-060 (`R-005`, `AC-004`) — evidence: `cargo test -p pgtui --test screen_browser_test` prints `2 passed`.
    - [ ] **4.2** Lint is clean (`D-004`) — evidence: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] **5** Gate passes.
    - [ ] **5.1** Regression `AC-005` holds and only `expected_paths` changed — evidence: the combined `cargo test -p pgtui --test ...` command of `AC-005` prints `48 passed`, and `git status --porcelain` lists only in-scope files.
    - [ ] **5.2** Gate green (`AC-006`) — evidence: `taskfmt verify` exits 0 with last line `DONE`.
<!-- checklist:end -->
