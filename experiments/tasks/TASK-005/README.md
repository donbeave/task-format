---
schema: task/v4
id: TASK-005
title: "Preview grid with client-side sort"
kind: feature
verify: "taskfmt verify"
expected_paths:
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

- `grid.rs` implements D-050..D-053 (model, comparison, null placement, client-side sort); `PgSession::query` returns `QueryOutcome` for the D-025 preview SQL; `app.rs` handles `Effect::Query`/`Msg::QueryDone` for `Preview`; `ui/grid.rs` renders the D-060 grid. Custom SQL and `x` stay inert (TASK-006); `d` stays inert (TASK-007).

Read before editing (in order):

1. `/task/decisions.md` — D-050..D-053 (grid) and D-025 (preview SQL) are the contract for this task.
2. `crates/pgtui/tests/grid_sort_test.rs` — the pure sort oracle: stable ties, numeric vs byte-wise compare, null placement, sort reset.
3. `crates/pgtui/tests/pg_preview_test.rs`, `app_preview_test.rs`, `screen_preview_test.rs` — live-server, state and rendering oracles.

Code flow: `Enter` on a sidebar row stores the pending `TableRef` and emits `Effect::Query { kind: Preview(t), sql }`; `runtime::execute` calls `PgSession::query(sql)`, which runs the exact D-025 preview SQL and replies `Msg::QueryDone { kind, result }`. `Ok(QueryOutcome::Rows(rs))` builds `Grid::from(rs)` into `app.grid`, focuses the grid and resets cursors/sort; `Err` keeps any existing grid and sets `Status::Error`. `s` cycles the sort on the cursor column (D-052) and `visible_rows()` feeds the renderer.

Baseline, from the repo root:

```sh
cargo test -p pgtui --test grid_sort_test
```

Expected before this change: a compile error — `pgtui::grid` does not exist.

## Preconditions

If a command fails, stop and report `BLOCKED`. Never work around a precondition. Docker is required.

- **P-001:** toolchain available — `cargo --version`
- **P-002:** TASK-004 gate green — `taskfmt verify`
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
- **R-007 (MUST NOT):** Add, remove, or bump a dependency; use `sort_unstable`; touch `render.rs`, `fonts/`, or anything under `crates/pgtui/tests/`; implement custom SQL, `x`, `d`, or the gallery; create `custom_sql.rs`, `justfile`, or `README.md`.

## Acceptance criteria

Observable behaviour plus the exact evidence command. The gate runs these; the harness re-runs them.

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given result sets from the seed and synthetic tables, when `Grid` sorts them, then the D-051/D-052 ordering, stability, reset and empty cases hold exactly. | `cargo test -p pgtui --test grid_sort_test` | exit 0, `8 passed` |
| AC-002 | Given a seeded container, when previews run, then rows match `fake_data` cell-for-cell, NULLs survive, the 500 limit applies on a 600-row table, and an unknown table yields `DbError::Query`. | `cargo test -p pgtui --test pg_preview_test` | exit 0, `4 passed` |
| AC-003 | Given a connected `App`, when `Enter` and grid keys are applied, then the preview effect, grid build, cursor clamping, sort cycling and error path behave as decided. | `cargo test -p pgtui --test app_preview_test` | exit 0, `8 passed` |
| AC-004 | Given grid state, when the 100x30 buffer is rendered, then headers, `[cursor column]`, sort arrows, rows, status and help appear as D-060 states. | `cargo test -p pgtui --test screen_preview_test` | exit 0, `5 passed` |
| AC-005 | Given the whole task, when all earlier trusted tests run, then they still pass. | `cargo test -p pgtui --test skeleton_test --test store_test --test app_connection_list_test --test screen_connection_list_test --test cli_test --test app_create_form_test --test runtime_create_test --test screen_create_form_test --test pg_connect_test --test app_browser_test --test pg_runtime_connect_test --test screen_browser_test --test grid_sort_test --test pg_preview_test --test app_preview_test --test screen_preview_test` | exit 0, `73 passed` |
| AC-006 | Given the finished task, when the gate runs, then it reports `DONE`. | `taskfmt verify` | exit 0, last line `DONE` |

## Fixed decisions

Implement; do not reopen. Verbatim text in `/task/decisions.md`. Anything not decided there that changes public behaviour, architecture, data or security posture is `NEEDS_REPLAN`, not executor discretion.

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
    - [ ] **2.1** D-050 model with order-preserving `from` and `visible_rows` (`R-001`, `AC-001`) — evidence: `cargo test -p pgtui --test grid_sort_test` prints `8 passed`.
    - [ ] **2.2** D-051/D-052 comparison, stability and null placement (`R-002`) — evidence: `grep -q 'sort_by' crates/pgtui/src/grid.rs && ! grep -q 'sort_unstable' crates/pgtui/src` exits 0.
- [ ] **3** Preview path is implemented.
    - [ ] **3.1** `PgSession::query` with the exact D-025 preview SQL (`R-004`, `AC-002`) — evidence: `cargo test -p pgtui --test pg_preview_test` prints `4 passed`.
    - [ ] **3.2** `Effect::Query`/`Msg::QueryDone` handling and grid keys (`R-005`, `AC-003`) — evidence: `cargo test -p pgtui --test app_preview_test` prints `8 passed`.
- [ ] **4** Rendering is implemented.
    - [ ] **4.1** `ui/grid.rs` and browser main pane per D-060 (`R-006`, `AC-004`) — evidence: `cargo test -p pgtui --test screen_preview_test` prints `5 passed`.
    - [ ] **4.2** Lint is clean (`D-004`) — evidence: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] **5** Gate passes.
    - [ ] **5.1** Regression `AC-005` holds and only `expected_paths` changed — evidence: the combined `cargo test -p pgtui --test ...` command of `AC-005` prints `73 passed`, and `git status --porcelain` lists only in-scope files.
    - [ ] **5.2** Gate green (`AC-006`) — evidence: `taskfmt verify` exits 0 with last line `DONE`.
<!-- checklist:end -->
