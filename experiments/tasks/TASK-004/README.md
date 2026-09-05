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

- TASK-003 completed the create form; `Enter` on the connection list is still a no-op, `db/mod.rs` contains only D-027 placeholder types, `db/postgres.rs` and the browser screen do not exist, and `pgtui` does not link `tokio-postgres` into `src/`.
- Trusted tests `pg_connect_test.rs`, `pg_runtime_connect_test.rs`, `app_browser_test.rs`, `screen_browser_test.rs` are committed on the run base commit and fail to compile; `tests/fixtures/seed.sql` and `tests/support/fake_data.rs` are already there.

Desired behavior:

- `db/mod.rs` with the D-026 types and `quote_ident`; `db/postgres.rs` with `PgSession::connect`/`list_tables` per D-023..D-025; `Effect::Connect` handled in `runtime.rs`; `SessionView` and the browser sidebar per D-010/D-033/D-060. Preview, `x`, and `d` stay inert (later tasks).

Read before editing (orientation only, non-normative, in order):

1. `crates/pgtui/tests/pg_connect_test.rs` and `crates/pgtui/tests/pg_runtime_connect_test.rs` — the live-server oracles; they start `postgres:16-alpine` with `fixtures/seed.sql`.
2. `crates/pgtui/tests/app_browser_test.rs` and `crates/pgtui/tests/screen_browser_test.rs` — state and rendering oracles for the sidebar.

Code flow: `Enter` on a non-empty list emits `Effect::Connect(saved)`; `runtime::execute` builds `ConnParams::from(&saved)`, calls `PgSession::connect` (5 s timeout, D-024), then `list_tables` (D-025), and replies `Msg::Connected(..)`. `Ok` fills `session: Some(SessionView)` and switches to `Screen::Browser`; `Err` keeps the list and sets `Status::Error`. `db/postgres.rs` is the only place `tokio_postgres` is named; every statement goes through `simple_query`.

Baseline, from the repo root:

```sh
cargo test -p pgtui --test pg_connect_test
```

Expected before this change: a compile error — `pgtui::db::PgSession` is unresolved because TASK-003 provides only the D-027 `db/mod.rs` placeholder.

## Preconditions

If a command fails, stop and report `BLOCKED`. Never work around a precondition. Docker is required from this task on.

- **P-001:** toolchain available — `cargo --version`
- **P-002:** TASK-003 workspace builds — `cargo build --workspace --lib --bins`
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

Canonical typed acceptance blocks use taskfmt's Markdown profile. They are task metadata, not
Cucumber feature files and have no runtime step definitions.

### AC-001 — Seed tables are listed in order
```gherkin
Given a seeded postgres:16-alpine container
When PgSession connects and lists tables
Then the four seed tables are returned in schema order through the decided query path
```

**Verification**

- **Type:** scenario
- **Covers:** `R-001, R-002, R-003, R-004`
- **Expected:** exit 0, `4 passed`

```sh
cargo test -p pgtui --test pg_connect_test
```

### AC-002 — Connection failures map to decided errors
```gherkin
Given a refused port or an invalid database password
When PgSession attempts to connect
Then the corresponding decided error is returned without terminating the list flow
```

**Verification**

- **Type:** scenario
- **Covers:** `R-003`
- **Expected:** exit 0, `4 passed`

```sh
cargo test -p pgtui --test pg_connect_test
```

### AC-003 — Runtime connection returns tables
```gherkin
Given a live database session
When Effect::Connect is executed
Then runtime replies with Msg::Connected(Ok(tables)) containing the listed tables
```

**Verification**

- **Type:** scenario
- **Covers:** `R-003`
- **Expected:** exit 0, `1 passed`

```sh
cargo test -p pgtui --test pg_runtime_connect_test
```

### AC-004 — Browser transitions and navigation behave
```gherkin
Given the connection list receives a successful or failed connection result
When browser keys and connection effects are applied
Then success enters the browser, failure stays on the list, and sidebar cursors clamp
```

**Verification**

- **Type:** scenario
- **Covers:** `R-005`
- **Expected:** exit 0, `6 passed`

```sh
cargo test -p pgtui --test app_browser_test
```

### AC-005 — Browser rendering follows D-060
```gherkin
Given a connected App with table rows
When the 100x30 browser buffer is rendered
Then schema.name rows, the selected marker, and the focused-pane marker appear as decided
```

**Verification**

- **Type:** scenario
- **Covers:** `R-005`
- **Expected:** exit 0, `2 passed`

```sh
cargo test -p pgtui --test screen_browser_test
```

### AC-006 — Earlier trusted behavior remains green
```gherkin
The complete trusted suite for the task remains green.
```

**Verification**

- **Type:** invariant
- **Covers:** `R-001, R-002, R-003, R-004, R-005`
- **Expected:** exit 0, 12 `test result: ok.` lines (one per target), no `FAILED`

```sh
cargo test -p pgtui \
  --test store_test \
  --test app_connection_list_test \
  --test screen_connection_list_test \
  --test cli_test \
  --test app_create_form_test \
  --test runtime_create_test \
  --test screen_create_form_test \
  --test pg_connect_test \
  --test pg_runtime_connect_test \
  --test app_browser_test \
  --test screen_browser_test \
  --test skeleton_test -- --skip pgtui_stub_exits_2
```

### AC-007 — Completion gate passes
**Verification**

- **Type:** gate
- **Expected:** exit 0, last line `DONE`

```sh
taskfmt verify
```

## Fixed decisions

Full text: `/task/decisions.md` (binding, read-only).
Implement; do not reopen. Anything not decided there that changes public behaviour, architecture, data or security posture is `NEEDS_REPLAN`, not executor discretion.

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
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test pg_connect_test` fails to compile with `pgtui::db::PgSession` unresolved.
- [ ] **2** Database layer is implemented.
    - [ ] **2.1** D-026 types and `quote_ident` in `db/mod.rs` (`R-001`) — evidence: `grep -q 'pub const PREVIEW_LIMIT: usize = 500' crates/pgtui/src/db/mod.rs && grep -q 'pub fn quote_ident' crates/pgtui/src/db/mod.rs` exits 0.
    - [ ] **2.2** Connection setup is complete.
        - [ ] **2.2.1** Seed tables are listed in order (`R-002`, `R-004`, `AC-001`) — evidence: `cargo test -p pgtui --test pg_connect_test` prints `4 passed` for table order.
        - [ ] **2.2.2** Connection failures map to decided errors (`R-003`, `AC-002`) — evidence: `cargo test -p pgtui --test pg_connect_test` prints `4 passed` for failure mapping.
    - [ ] **2.3** Only `simple_query` is used under `db/` (`R-002`) — evidence: `! grep -rnE '\.query\(|query_one|query_raw|\.prepare\(|\.execute\(' crates/pgtui/src/db` exits 0.
- [ ] **3** Connect flow is wired.
    - [ ] **3.1** `Effect::Connect` executes and replies `Msg::Connected` (`AC-003`) — evidence: `cargo test -p pgtui --test pg_runtime_connect_test` prints `1 passed`.
    - [ ] **3.2** Browser state transitions and sidebar cursors per D-033/D-010 (`AC-004`) — evidence: `cargo test -p pgtui --test app_browser_test` prints `6 passed`.
- [ ] **4** Rendering is implemented.
    - [ ] **4.1** `ui/browser.rs` sidebar per D-060 (`R-005`, `AC-005`) — evidence: `cargo test -p pgtui --test screen_browser_test` prints `2 passed`.
    - [ ] **4.2** Lint is clean (`D-004`) — evidence: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] **5** Gate passes.
    - [ ] **5.1** Regression `AC-006` holds — evidence: the `AC-006` command exits 0 and prints 12 `test result: ok.` lines, one per `--test` target, and no `FAILED`.
    - [ ] **5.2** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated (`R-006`, `R-007`) — evidence: `git status --porcelain` and `git diff --no-renames --stat $TASKFMT_BASE` show only in-scope files.
    - [ ] **5.3** `taskfmt verify` exits 0 with last line `DONE` (`AC-007`) — evidence: final full run (with progress check), full output shown in the transcript.
<!-- checklist:end -->
