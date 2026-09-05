---
schema: task/v4
id: TASK-005
title: "Preview grid with client-side sort"
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
  - "crates/pgtui/src/grid.rs"
  - "crates/pgtui/src/ui/mod.rs"
  - "crates/pgtui/src/ui/browser.rs"
  - "crates/pgtui/src/ui/grid.rs"
---

# TASK-005 — Preview grid with client-side sort

Execution protocol, progress grammar, report format: `/task/AGENTS.md`. This file is read-only and states WHAT must become true.

## Goal

`Enter` on a sidebar table fetches `SELECT * FROM "<schema>"."<table>" LIMIT 500`, renders a shared grid widget, and `s` sorts the loaded rows client-side with PostgreSQL null placement.

## Context

Current behavior:

- TASK-004 connects and lists tables; the browser's main pane is an empty block, `Enter` on the sidebar is inert, and no grid exists.
- Trusted tests `grid_sort_test.rs`, `pg_preview_test.rs`, `app_preview_test.rs`, `screen_preview_test.rs` are committed on the run base commit and fail to compile.

Desired behavior:

- Preview grid and client-side sort (D-050..D-053, D-025, D-060) behave exactly as R-001..R-006 state and AC-001..AC-005 prove. Custom SQL and `x` stay inert (TASK-006); `d` stays inert (TASK-007).

Read before editing (orientation only, non-normative, in order):

1. `crates/pgtui/tests/grid_sort_test.rs` — the pure sort oracle: stable ties, numeric vs byte-wise compare, null placement, sort reset.
2. `crates/pgtui/tests/pg_preview_test.rs`, `app_preview_test.rs`, `screen_preview_test.rs` — live-server, state and rendering oracles.

Code flow: `Enter` on a sidebar row stores the pending `TableRef` and emits `Effect::Query { kind: Preview(t), sql }`; `runtime::execute` calls `PgSession::query(sql)`, which runs the exact D-025 preview SQL and replies `Msg::QueryDone { kind, result }`. `Ok(QueryOutcome::Rows(rs))` builds `Grid::from(rs)` into `app.grid`, focuses the grid and resets cursors/sort; `Err` keeps any existing grid and sets `Status::Error`. `s` cycles the sort on the cursor column (D-052) and `visible_rows()` feeds the renderer.

Baseline, from the repo root:

```sh
cargo test -p pgtui --test grid_sort_test
```

Expected before this change: a compile error — `pgtui::grid` does not exist.

## Preconditions

If a command fails, stop and report `BLOCKED`. Never work around a precondition. Docker is required.

- **P-001:** toolchain available — `cargo --version`
- **P-002:** TASK-004 suites green — `cargo test -p pgtui --test app_browser_test --test screen_browser_test`
- **P-003:** docker reachable — `docker info >/dev/null`
- **P-004:** trusted grid tests present — `test -f crates/pgtui/tests/grid_sort_test.rs`
- **P-005:** working tree clean — `test -z "$(git status --porcelain)"`

## Scope

In scope:

- `grid.rs`, `ui/grid.rs`, `PgSession::query` in `db/postgres.rs`, preview effect handling in `app.rs`/`runtime.rs`, grid keys in `keys.rs`, browser main pane in `ui/browser.rs`.

Out of scope:

- Custom SQL, `custom_sql.rs`, the `x` key; TASK-006.
- Disconnect (`d`), exit-code work, gallery, `README.md`.
- `render.rs`, `fonts/`, anything under `crates/pgtui/tests/`, the store and connect contracts.

## Requirements

- **R-001 (MUST):** Grid per D-050: private `rows`, pub `columns`/`col_cursor`/`row_cursor`/`sort`, `Grid::from(ResultSet)` keeping order, `visible_rows()` returning the sorted view without mutating the original.
- **R-002 (MUST):** Comparison per D-051 and placement per D-052: numeric when every non-null cell parses as `f64`, else byte-wise; stable `sort_by`; asc NULLs last, desc NULLs first; `s` cycles `None -> Asc -> Desc -> None` on the cursor column and starts at `Asc` on a different column; a new preview resets sort and cursors.
- **R-003 (MUST):** Sort never re-queries: it sorts the fetched rows only (D-053), and only stable sorts are used.
- **R-004 (MUST):** Preview SQL is exactly `SELECT * FROM "<schema>"."<table>" LIMIT 500` built with `quote_ident` (D-025); `PgSession::query` returns `Result<QueryOutcome, DbError>` through `simple_query`.
- **R-005 (MUST):** App per D-011/D-033: `QueryDone(Ok(Rows))` builds the grid, focuses `Focus::Grid`, resets cursors; `QueryDone(Err)` keeps the previous grid and sets `Status::Error`; grid keys `h`/`l` and `j`/`k` clamp; `Tab` toggles only when a grid is loaded.
- **R-006 (MUST):** Rendering per D-060: header row, `[cursor column]`, ` ^`/` v` sort suffix, column width `clamp(max(len(header)+4, max cell len), 4, 24)`, `> ` on the cursor row, `Cell::Null` as `NULL`, main block title ` <schema.table>  <rows> rows  limit 500 `.
- **R-007 (MUST NOT):** Remove or bump a D-001 pin, or depend on anything outside the D-001 pin list (adding a listed pin is required where trusted tests need it); use `sort_unstable`; touch `render.rs`, `fonts/`, or anything under `crates/pgtui/tests/`; implement custom SQL, `x`, `d`, or the gallery; create `custom_sql.rs`, `justfile`, or `README.md`.

## Acceptance criteria

Canonical typed acceptance blocks use taskfmt's Markdown profile. They are task metadata, not
Cucumber feature files and have no runtime step definitions.

### AC-001 — Grid ordering and null placement are correct
```gherkin
Given result sets from seed and synthetic tables
When Grid sorts the fetched rows
Then numeric and byte-wise ordering, stable ties, and D-052 null placement match the contract
```

**Verification**

- **Type:** scenario
- **Covers:** `R-001, R-002, R-003`
- **Expected:** exit 0, `8 passed`

```sh
cargo test -p pgtui --test grid_sort_test
```

### AC-002 — Grid reset and empty cases are safe
```gherkin
The grid resets sort and cursors for a new preview and handles an empty result without mutation.
```

**Verification**

- **Type:** invariant
- **Covers:** `R-001, R-002, R-003`
- **Expected:** exit 0, `8 passed`

```sh
cargo test -p pgtui --test grid_sort_test
```

### AC-003 — Preview rows preserve data and apply the cap
```gherkin
Given a seeded database and a previewable table
When a preview query runs
Then rows match the fixture cell-for-cell, NULLs survive, and 600 rows are capped at 500
```

**Verification**

- **Type:** scenario
- **Covers:** `R-004`
- **Expected:** exit 0, `4 passed`

```sh
cargo test -p pgtui --test pg_preview_test
```

### AC-004 — Unknown preview tables return a query error
```gherkin
Given a table name that does not exist
When a preview query runs
Then the database failure maps to DbError::Query
```

**Verification**

- **Type:** scenario
- **Covers:** `R-004`
- **Expected:** exit 0, `4 passed`

```sh
cargo test -p pgtui --test pg_preview_test
```

### AC-005 — Preview results build and focus the grid
```gherkin
Given a connected App and a selected table
When Enter completes a preview query
Then the result builds the grid, focuses it, and clamps its row cursor
```

**Verification**

- **Type:** scenario
- **Covers:** `R-005`
- **Expected:** exit 0, `8 passed`

```sh
cargo test -p pgtui --test app_preview_test
```

### AC-006 — Grid keys and failures preserve state
```gherkin
Given a loaded grid or a previous grid with a query failure
When grid navigation, sort, Tab, or the error result is applied
Then sorting cycles as decided and a failed query keeps the previous grid with an error status
```

**Verification**

- **Type:** scenario
- **Covers:** `R-005`
- **Expected:** exit 0, `8 passed`

```sh
cargo test -p pgtui --test app_preview_test
```

### AC-007 — Preview rendering follows D-060
```gherkin
Given grid state with rows and a selected column
When the 100x30 preview buffer is rendered
Then headers, cursor and sort markers, rows, status, and help appear as decided
```

**Verification**

- **Type:** scenario
- **Covers:** `R-006`
- **Expected:** exit 0, `5 passed`

```sh
cargo test -p pgtui --test screen_preview_test
```

### AC-008 — Earlier trusted behavior remains green
```gherkin
The complete trusted suite for the task remains green.
```

**Verification**

- **Type:** invariant
- **Covers:** `R-001, R-002, R-003, R-004, R-005, R-006`
- **Expected:** exit 0, 16 `test result: ok.` lines (one per target), no `FAILED`

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
  --test grid_sort_test \
  --test pg_preview_test \
  --test app_preview_test \
  --test screen_preview_test \
  --test skeleton_test -- --skip pgtui_stub_exits_2
```

### AC-009 — Completion gate passes
**Verification**

- **Type:** gate
- **Expected:** exit 0, last line `DONE`

```sh
taskfmt verify
```

## Fixed decisions

Full text: `/task/decisions.md` (binding, read-only).
Implement; do not reopen. Anything not decided there that changes public behaviour, architecture, data or security posture is `NEEDS_REPLAN`, not executor discretion.

- **D-050..D-053:** grid model, comparison, null placement and cycling, client-side sort.
- **D-025:** exact preview SQL, `PREVIEW_LIMIT`, no `ORDER BY`.
- **D-011, D-033:** `QueryDone`/`Query` round trip and grid keys.
- **D-060:** grid widget and browser main pane layout.
- **D-041, D-072:** error surfacing; seed fixture the trusted containers apply.
- **D-070, D-071:** trusted helpers and behavioural verification.

## Checklist

Static plan. Hierarchical IDs, four spaces per level, max depth 4. Every leaf names what becomes true and the evidence permitting the check.

<!-- checklist:start -->
- [ ] **1** Baseline and environment are reproduced.
    - [ ] **1.1** Preconditions `P-001..P-005` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test grid_sort_test` fails to compile with `pgtui::grid` unresolved.
- [ ] **2** Grid model is implemented.
    - [ ] **2.1** Grid ordering and null placement are correct (`R-001`, `R-002`, `R-003`, `AC-001`) — evidence: `cargo test -p pgtui --test grid_sort_test` prints `8 passed` for ordering.
    - [ ] **2.2** D-051/D-052 comparison, stability and null placement, stable sort only (`R-002`, `R-003`, `R-007`, `AC-002`) — evidence: `grep -q 'sort_by' crates/pgtui/src/grid.rs && ! grep -q 'sort_unstable' crates/pgtui/src` exits 0.
- [ ] **3** Preview path is implemented.
    - [ ] **3.1** Preview data behavior is complete (`R-004`).
        - [ ] **3.1.1** Preview rows preserve data and apply the cap (`AC-003`) — evidence: `cargo test -p pgtui --test pg_preview_test` prints `4 passed` for rows and cap.
        - [ ] **3.1.2** Unknown preview tables return a query error (`AC-004`) — evidence: `cargo test -p pgtui --test pg_preview_test` prints `4 passed` for error mapping.
    - [ ] **3.2** Preview app behavior is complete (`R-005`).
        - [ ] **3.2.1** Preview results build and focus the grid (`AC-005`) — evidence: `cargo test -p pgtui --test app_preview_test` prints `8 passed` for result state.
        - [ ] **3.2.2** Grid keys and failures preserve state (`AC-006`) — evidence: `cargo test -p pgtui --test app_preview_test` prints `8 passed` for keys and errors.
- [ ] **4** Rendering is implemented.
    - [ ] **4.1** `ui/grid.rs` and browser main pane per D-060 (`R-006`, `AC-007`) — evidence: `cargo test -p pgtui --test screen_preview_test` prints `5 passed`.
    - [ ] **4.2** Lint is clean (`D-004`) — evidence: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] **5** Gate passes.
    - [ ] **5.1** Regression `AC-008` holds — evidence: the `AC-008` command exits 0 and prints 16 `test result: ok.` lines, one per `--test` target, and no `FAILED`.
    - [ ] **5.2** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated (`R-007`) — evidence: `git status --porcelain` and `git diff --no-renames --stat $TASKFMT_BASE` show only in-scope files.
    - [ ] **5.3** `taskfmt verify` exits 0 with last line `DONE` (`AC-009`) — evidence: final full run (with progress check), full output shown in the transcript.
<!-- checklist:end -->
