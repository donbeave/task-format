---
schema: task/v4
id: TASK-006
title: "Custom SQL screen"
kind: feature
verify: "taskfmt verify"
expected_paths:
  - "Cargo.toml"
  - "Cargo.lock"
  - "crates/pgtui/Cargo.toml"
  - "crates/pgtui/src/app.rs"
  - "crates/pgtui/src/keys.rs"
  - "crates/pgtui/src/runtime.rs"
  - "crates/pgtui/src/db/mod.rs"
  - "crates/pgtui/src/db/postgres.rs"
  - "crates/pgtui/src/ui/mod.rs"
  - "crates/pgtui/src/ui/custom_sql.rs"
  - "crates/pgtui/src/ui/grid.rs"
---

# TASK-006 — Custom SQL screen

Execution protocol, progress grammar, report format: `/task/AGENTS.md`. This file is read-only and states WHAT must become true.

## Goal

`x` opens a SQL input screen whose statement is run verbatim, row results render in an unsortable grid, row counts and affected-row counts surface as status info, and multi-statement input is rejected.

## Context

Current behavior:

- TASK-005 shipped the preview grid; the browser's `x` key is still inert, `sql_input`/`sql_grid` exist as fields only, and `QueryKind::Custom` is never produced.
- Trusted tests `app_custom_sql_test.rs`, `pg_custom_sql_test.rs`, `screen_custom_sql_test.rs` are committed on the run base commit and fail to compile.

Desired behavior:

- Custom SQL (D-034), its D-025 semantics and the D-060 layout with the unsortable grid behave exactly as R-001..R-005 state and AC-001..AC-004 prove.

Read before editing (orientation only, non-normative, in order):

1. `crates/pgtui/tests/pg_custom_sql_test.rs` — the live-server oracle: `SELECT`, `CommandComplete` affected counts, cap, multi-statement rejection, syntax errors.
2. `crates/pgtui/tests/app_custom_sql_test.rs` and `crates/pgtui/tests/screen_custom_sql_test.rs` — state and rendering oracles.

Code flow: `keys.rs` routes editing keys while `screen == CustomSql`; `Enter` trims the input and emits the effect. `runtime::execute` calls `PgSession::query(sql)`, which runs `simple_query`, maps multiple row descriptions to `DbError::MultiStatement`, counts `CommandComplete` rows for `Affected`, and caps rows client-side at `PREVIEW_LIMIT`. `app.rs` stores row results in `sql_grid` (sort stays `None`, `col_cursor` stays `0`) and sets the decided `Status::Info` strings; `ui/custom_sql.rs` draws the input and results blocks.

Baseline, from the repo root:

```sh
cargo test -p pgtui --test pg_custom_sql_test
```

Expected before this change: a compile error — `Screen::CustomSql` is never entered and `CustomSql`-specific handling is missing.

## Preconditions

If a command fails, stop and report `BLOCKED`. Never work around a precondition. Docker is required.

- **P-001:** toolchain available — `cargo --version`
- **P-002:** TASK-005 suites green — `cargo test -p pgtui --test grid_sort_test --test app_preview_test --test screen_preview_test`
- **P-003:** docker reachable — `docker info >/dev/null`
- **P-004:** trusted custom-SQL tests present — `test -f crates/pgtui/tests/pg_custom_sql_test.rs`
- **P-005:** working tree clean — `test -z "$(git status --porcelain)"`

## Scope

In scope:

- `ui/custom_sql.rs`, CustomSql keys in `keys.rs`, custom-SQL state handling in `app.rs`, `PgSession::query` semantics for `Custom`, the shared grid widget as used by `sql_grid`.

Out of scope:

- Disconnect (`d`), the exit-code contract, gallery, `README.md`.
- Preview behaviour and sort; both are frozen by TASK-005.
- `render.rs`, `fonts/`, anything under `crates/pgtui/tests/`, the store and connect contracts.

## Requirements

- **R-001 (MUST):** Keys per D-034: printable chars append, `Backspace` pops, `Enter` on empty input is a no-op, `Enter` on input emits `Effect::Query { kind: Custom, sql }`, `Esc` returns to `Screen::Browser` retaining session, grid and input; no `d`/`q` on this screen, `Ctrl+C` still global.
- **R-002 (MUST):** SQL per D-025: whitespace and one trailing `;` trimmed, more than one row description -> `DbError::MultiStatement`, everything through `simple_query`.
- **R-003 (MUST):** Results per D-025/D-011: `Rows` capped at `PREVIEW_LIMIT` with `Status::Info("showing first 500 rows")` when capped; `Affected(n)` -> `Status::Info("ok: <n> rows affected")` and `sql_grid = None`.
- **R-004 (MUST):** `sql_grid` reuses `Grid` with `sort == None` and `col_cursor == 0`; `Up`/`Down` move the row cursor only; `s` and `h`/`l` type into the input instead (D-034).
- **R-005 (MUST):** Rendering per D-060: input block ` SQL ` with `> <sql_input>`, results block ` Results ` plus `Type a query and press Enter` while empty, ` Results  <rows> rows ` after a result, help `Enter run  Up/Down rows  Esc back  Ctrl+C quit`, and no `[ ]`/sort markers on the custom grid.
- **R-006 (MUST):** Final design directly; no compatibility layer, dual path, deprecated alias, feature flag, or legacy fallback.
- **R-007 (MUST NOT):** Remove or bump a D-001 pin, or depend on anything outside the D-001 pin list (adding a listed pin is required where trusted tests need it); use `sort_unstable`; touch `render.rs`, `fonts/`, or anything under `crates/pgtui/tests/`; change preview or store behaviour; implement `d`, the gallery, or `README.md`.

## Acceptance criteria

Observable behaviour plus the exact evidence command. The gate runs these; the harness re-runs them.

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given a connected `App`, when `x` and editing keys are applied, then the screen opens, input edits correctly, empty `Enter` is inert, `Enter` emits the custom query effect, results build the grid with the decided status, and `Esc` retains the input. | `cargo test -p pgtui --test app_custom_sql_test` | exit 0, `9 passed` |
| AC-002 | Given a seeded container, when custom statements run, then `SELECT` returns rows, `CommandComplete` reports affected counts, multi-statement input is rejected, rows are capped, and syntax errors map to `DbError::Query`. | `cargo test -p pgtui --test pg_custom_sql_test` | exit 0, `5 passed` |
| AC-003 | Given custom-SQL state, when the 100x30 buffer is rendered, then the input block, hint, result title, echoed SQL and marker-free grid appear as D-060 states. | `cargo test -p pgtui --test screen_custom_sql_test` | exit 0, `3 passed` |
| AC-004 | Given the whole task, when all earlier trusted tests run, then they still pass. | `cargo test -p pgtui --test store_test --test app_connection_list_test --test screen_connection_list_test --test cli_test --test app_create_form_test --test runtime_create_test --test screen_create_form_test --test pg_connect_test --test pg_runtime_connect_test --test app_browser_test --test screen_browser_test --test grid_sort_test --test pg_preview_test --test app_preview_test --test screen_preview_test --test app_custom_sql_test --test pg_custom_sql_test --test screen_custom_sql_test` | exit 0, `86 passed` |
| AC-005 | Given the finished task, when the gate runs, then it reports `DONE`. | `taskfmt verify` | exit 0, last line `DONE` |

## Fixed decisions

Full text: `/task/decisions.md` (binding, read-only).
Implement; do not reopen. Anything not decided there that changes public behaviour, architecture, data or security posture is `NEEDS_REPLAN`, not executor discretion.

- **D-034:** CustomSql keys, entry via `x`, retention on `Esc`.
- **D-025:** custom-SQL trimming, single-statement rule, cap and status strings.
- **D-011, D-013:** `QueryOutcome`/`QueryDone` round trip and status line.
- **D-060:** CustomSql layout and the marker-free grid.
- **D-050, D-041:** frozen grid model; error surfacing.
- **D-070, D-071:** trusted helpers and behavioural verification.

## Checklist

Static plan. Hierarchical IDs, four spaces per level, max depth 4. Every leaf names what becomes true and the evidence permitting the check.

<!-- checklist:start -->
- [ ] **1** Baseline and environment are reproduced.
    - [ ] **1.1** Preconditions `P-001..P-005` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test pg_custom_sql_test` fails to compile or reports `CustomSql` unreachable.
- [ ] **2** Screen state is implemented.
    - [ ] **2.1** `x` opens CustomSql and editing keys behave per D-034 (`R-001`, `AC-001`) — evidence: `cargo test -p pgtui --test app_custom_sql_test` prints `9 passed`.
    - [ ] **2.2** `sql_grid` uses the frozen D-050 model with no sort and no column cursor (`R-004`, `R-007`) — evidence: `grep -q 'sql_grid' crates/pgtui/src/app.rs && ! grep -q 'sort_unstable' crates/pgtui/src` exits 0.
- [ ] **3** Query semantics are implemented.
    - [ ] **3.1** D-025 custom-SQL handling in `db/postgres.rs` (`R-002`, `R-003`, `AC-002`) — evidence: `cargo test -p pgtui --test pg_custom_sql_test` prints `5 passed`.
    - [ ] **3.2** Cap and affected-row status strings match D-025 exactly (`R-003`) — evidence: `grep -q 'showing first 500 rows' crates/pgtui/src && grep -q 'rows affected' crates/pgtui/src` exits 0.
- [ ] **4** Rendering is implemented.
    - [ ] **4.1** `ui/custom_sql.rs` per D-060 (`R-005`, `AC-003`) — evidence: `cargo test -p pgtui --test screen_custom_sql_test` prints `3 passed`.
    - [ ] **4.2** Lint is clean (`D-004`) — evidence: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] **5** Gate passes.
    - [ ] **5.1** Regression `AC-004` holds — evidence: the combined `cargo test -p pgtui --test ...` command of `AC-004` prints `86 passed`.
    - [ ] **5.2** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated (`R-006`, `R-007`) — evidence: `git status --porcelain` and `git diff --no-renames --stat $TASKFMT_BASE` show only in-scope files.
    - [ ] **5.3** `taskfmt verify` exits 0 with last line `DONE` (`AC-005`) — evidence: final full run (with progress check), full output shown in the transcript.
<!-- checklist:end -->
