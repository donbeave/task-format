---
schema: task/v3
id: TASK-104
title: "Preview a selected table in a sortable grid"
kind: feature
verify: "/task/verify.sh"
expected_paths:
  - "crates/pgtui/src/grid.rs"
  - "crates/pgtui/src/db/*"
  - "crates/pgtui/src/app.rs"
  - "crates/pgtui/src/keys.rs"
  - "crates/pgtui/src/runtime.rs"
  - "crates/pgtui/src/ui/browser.rs"
  - "crates/pgtui/src/ui/grid.rs"
  - "crates/pgtui/src/ui/mod.rs"
  - "crates/pgtui/src/lib.rs"
---

# TASK-104 — Preview a selected table in a sortable grid

Execution protocol, progress file grammar, and final report format are in `/task/AGENTS.md`. Verbatim fixed-decision text is in `/task/decisions.md`. Both are read-only, as is this file.

## Goal

`Enter` on a sidebar table shows its first 500 rows in the main pane, and `s` cycles the sort on the cursor column ascending → descending → unsorted.

## Context

Current behavior:

- `App::update` (`crates/pgtui/src/app.rs`) handles `Msg::Connected` and sidebar `j`/`k`; sidebar `Enter` is a no-op and `Tab` never changes focus. `Effect::Query` / `Msg::QueryDone` exist in the enums but are neither emitted nor handled.
- `PgSession` (`crates/pgtui/src/db/postgres.rs`) has `connect` and `list_tables` only. There is no `pgtui::grid` module and no `crates/pgtui/src/ui/grid.rs`.
- Trusted tests `grid_sort_test`, `pg_preview_test`, `app_preview_test`, `screen_preview_test` do not compile.

Desired behavior:

- Sidebar `Enter` emits `Effect::Query { kind: Preview(t), sql }` with the exact D-025 preview SQL; `runtime::execute` runs it via `PgSession::query` and replies `Msg::QueryDone`.
- `QueryDone(Ok)` installs a new `Grid` (sort `None`, cursors 0) and sets `Focus::Grid`; `QueryDone(Err)` sets `Status::Error` and keeps the previous grid.
- Grid keys move the cursors (clamped); `s` cycles the sort; the main pane renders per D-060 and matches the three protected snapshots.

Read before editing (non-normative hints, in order):

1. `AGENTS.md` — build, test, lint commands.
2. `/task/decisions.md` — D-025, D-026, D-033, D-041, D-050..D-053, D-060, D-061, D-070..D-072 verbatim; decided, do not reopen.
3. `crates/pgtui/src/app.rs`, `crates/pgtui/src/keys.rs` — `Msg`, `Effect`, `SessionView`, `Focus`, Browser key mapping to extend.
4. `crates/pgtui/src/db/postgres.rs` — how `list_tables` uses `simple_query`; `query` sits beside it.
5. `crates/pgtui/tests/support/fake_data.rs` — in-memory `ResultSet`s the snapshots render.
6. `crates/pgtui/tests/{grid_sort,app_preview,pg_preview,screen_preview}_test.rs` — the oracle; they compile against the D-026/D-050 signatures.

Code flow: `keys::action_for` maps a `KeyEvent` per screen and focus; `App::update` is pure and returns effects; `runtime::execute` performs IO with the `PgSession` opened in TASK-103 and returns the reply `Msg`, which `main.rs` feeds back into `update`. `ui::draw` dispatches to `ui/browser.rs`, which must render the shared grid widget (`ui/grid.rs`) in the main pane when `app.grid` is `Some`. Sorting is a view over `Grid` (`visible_rows`), never a re-query.

Baseline (run from repo root, before any edit):

```sh
cargo test -p pgtui --test grid_sort_test
```

Expected before this change: `error[E0432]: unresolved import` (`pgtui::grid` missing).

## Preconditions

Each precondition has a command. If a command fails, stop and report `BLOCKED` (see AGENTS.md). Do not work around it.

- **P-001:** toolchain available — `cargo --version`
- **P-002:** protected test support present — `test -f crates/pgtui/tests/support/mod.rs`
- **P-003:** baseline tag resolvable — `git rev-parse --verify baseline`
- **P-004:** Docker reachable (testcontainers) — `docker info >/dev/null`

## Scope

In scope:

- `grid.rs`: `Grid`, `SortState`, `SortDir`, comparator, `visible_rows`.
- `PgSession::query(&self, sql) -> Result<ResultSet, DbError>` over the simple-query protocol (row results; `CommandComplete` only needs to not error).
- `Effect::Query { Preview }` emission and execution; `Msg::QueryDone { Preview }` handling.
- Browser grid-focus keys (`Tab`, `h`/`l`, `j`/`k`, `s`) in `keys.rs`.
- `ui/grid.rs` widget and the main-pane title in `ui/browser.rs`.
- Executor unit tests for the comparator in `src/grid.rs`.

Out of scope:

- Custom SQL (`x` stays a no-op), disconnect (`d` no-op), server-side sort, pagination beyond 500, column resizing, keyring, dependency changes.

## Requirements

- **R-001 (MUST):** Preview SQL is exactly `SELECT * FROM "<schema>"."<table>" LIMIT 500` with identifiers double-quoted and embedded `"` doubled; `PREVIEW_LIMIT: usize = 500` is a `pub const` in `db/mod.rs`; no `ORDER BY`.
- **R-002 (MUST):** `Grid` matches D-050; `visible_rows` returns the sorted view and never mutates the source rows.
- **R-003 (MUST):** Sorting follows D-051/D-052: numeric compare when every non-null cell parses as `f64`, else byte-wise string compare; asc NULLs last, desc NULLs first; stable; `s` cycles `None → Asc → Desc → None` on the cursor column and restarts at `Asc` on another column; sort resets when a new preview loads.
- **R-004 (MUST):** Keys per D-033 grid focus; `Tab` is a no-op while `grid.is_none()`.
- **R-005 (MUST):** Grid renders per D-060: header `[name]` on the cursor column, ` ^`/` v` sort suffix, width `clamp(max(len(header)+4, max cell len), 4, 24)`, `NULL` for `Cell::Null`, `> ` row highlight; main title ` <schema.table>  <rows> rows  limit 500 ` with `*` on the focused pane.
- **R-006 (MUST):** `QueryDone(Err)` → `Status::Error` (D-041); previous grid and focus unchanged.
- **R-007 (MUST):** Implement the final design directly. No compatibility layer, dual path, feature flag, or fallback.
- **R-008 (MUST NOT):** Add or upgrade dependencies; edit `Cargo.toml`, `Cargo.lock`, `justfile`, `rust-toolchain.toml`, or anything under `crates/pgtui/tests/`; use `sort_unstable*`; use `#[ignore]`, `todo!`, `unimplemented!`, `dbg!`, or `#[allow(...)]`.

## Acceptance criteria

Each row is observable behavior with the exact evidence command. `verify.sh` runs these; the harness re-runs them.

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given `fake_data::preview(customers)`, when sorted by `balance` asc/desc, then numeric order, NULL placement per D-052, stable ties. | `cargo test -p pgtui --test grid_sort_test` | exit 0, `6 passed` |
| AC-002 | Given the seeded container, when `query` runs the preview SQL for each seed table, then row multisets and cells (incl. `Null`) equal `fake_data::preview(t)`; a 600-row table yields 500. | `cargo test -p pgtui --test pg_preview_test` | exit 0, `3 passed` |
| AC-003 | Given Browser with sidebar focus, when `Enter`, then `Effect::Query { Preview }` with the exact SQL; on `QueryDone(Ok)` focus is `Grid`, sort `None`; cursors clamp; `Tab` toggles only with a grid. | `cargo test -p pgtui --test app_preview_test -- --skip sort_cycle --skip query_error` | exit 0, `6 passed` |
| AC-004 | Given a grid, when `s` is pressed 1/2/3 times on a column, then `Asc`/`Desc`/`None`; `s` on another column → `Asc` there. | `cargo test -p pgtui --test app_preview_test sort_cycle` | exit 0, `2 passed` |
| AC-005 | Given fake customers loaded, when rendered unsorted, asc by balance, desc by balance, then `screen__preview_unsorted`, `screen__preview_sorted_asc`, `screen__preview_sorted_desc` (text + svg) match. | `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_preview_test` | exit 0, `6 passed` |
| AC-006 | Given `QueryDone(Err)`, when handled, then status is `error: ...` and the previous grid is unchanged. | `cargo test -p pgtui --test app_preview_test query_error_keeps_grid` | exit 0, `1 passed` |

## Fixed decisions

Already decided; full text in `/task/decisions.md`. Implement; do not reopen. Anything not listed that changes public behavior, architecture, data, or security posture is `NEEDS_REPLAN`, not executor discretion.

- **D-025:** preview SQL, `PREVIEW_LIMIT`, identifier quoting.
- **D-026:** `ResultSet`, `Cell`, `TableRef` types (exact).
- **D-033:** Browser keys incl. grid focus.
- **D-041:** every `Err` becomes `Status::Error`; no `unwrap`/`expect` on IO outside `main.rs` terminal setup.
- **D-050..D-053:** `Grid` model (exact struct), comparator, NULL placement and cycling, client-side sort.
- **D-060, D-061:** 100×30 frame, Browser and grid layout to the character, `ui::draw` entry point.
- **D-070..D-072:** protected test support, snapshot policy (`INSTA_UPDATE=no`, no `.snap.new`), seed data.

## Checklist

Static plan. IDs `N`…`N.N.N.N`, max depth 4, four spaces per level. Every leaf names what becomes true and its evidence. State lives in `progress.md`, never here.

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-004` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test grid_sort_test` fails to compile (`pgtui::grid` missing).
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Grid model and sorting (`R-002`, `R-003`, `D-050..D-053`) — evidence: `cargo test -p pgtui --test grid_sort_test` → `6 passed`.
        - [ ] **2.1.1** Numeric-vs-string column detection and comparator — evidence: `cargo test -p pgtui --test grid_sort_test numeric_ string_` → `2 passed`.
        - [ ] **2.1.2** NULL placement and stable ties — evidence: `cargo test -p pgtui --test grid_sort_test nulls_ stable_` → `3 passed`.
        - [ ] **2.1.3** Unit tests for the comparator in `src/grid.rs` — evidence: `cargo test -p pgtui --lib grid::tests` → `3 passed` or more.
    - [ ] **2.2** `PgSession::query` returns an all-text `ResultSet` and preview SQL matches `D-025` (`R-001`) — evidence: `cargo test -p pgtui --test pg_preview_test` → `3 passed`.
    - [ ] **2.3** App preview flow and grid keys (`R-004`, `R-006`, `D-033`) — evidence: `cargo test -p pgtui --test app_preview_test` → `9 passed`.
        - [ ] **2.3.1** `Enter` emits `Effect::Query { Preview }`; `QueryDone(Ok)` installs the grid and focuses it — evidence: `cargo test -p pgtui --test app_preview_test enter_ query_ok` → `3 passed`.
        - [ ] **2.3.2** `h`/`l`/`j`/`k` clamp; `s` cycles; `Tab` toggles focus only with a grid — evidence: `cargo test -p pgtui --test app_preview_test cursor_ sort_cycle tab_` → `5 passed`.
    - [ ] **2.4** Grid widget and main-pane title per `D-060` (`R-005`) — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_preview_test` → `6 passed`.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001` — evidence: `cargo test -p pgtui --test grid_sort_test` → `6 passed`.
    - [ ] **3.2** `AC-002` — evidence: `cargo test -p pgtui --test pg_preview_test` → `3 passed`.
    - [ ] **3.3** `AC-003`, `AC-004`, `AC-006` — evidence: `cargo test -p pgtui --test app_preview_test` → `9 passed`.
    - [ ] **3.4** `AC-005` — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_preview_test` → `6 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated — evidence: `git status --porcelain` and `git diff --stat baseline` show only in-scope files.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full verifier output shown in the transcript.
<!-- checklist:end -->
