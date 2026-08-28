---
schema: task/v3
id: TASK-105
title: "Run custom SQL from a dedicated screen into a plain grid"
kind: feature
verify: "/task/verify.sh"
expected_paths:
  - "crates/pgtui/src/app.rs"
  - "crates/pgtui/src/keys.rs"
  - "crates/pgtui/src/runtime.rs"
  - "crates/pgtui/src/db/*"
  - "crates/pgtui/src/ui/custom_sql.rs"
  - "crates/pgtui/src/ui/grid.rs"
  - "crates/pgtui/src/ui/mod.rs"
protected_paths:
  - "crates/pgtui/tests/*"
  - "crates/pgtui/src/store/*"
  - "crates/pgtui/src/grid.rs"
  - "crates/pgtui/src/render.rs"
  - "crates/pgtui/src/fonts/*"
  - "Cargo.toml"
  - "Cargo.lock"
  - "crates/pgtui/Cargo.toml"
  - "justfile"
  - "rust-toolchain.toml"
  - "CLAUDE.md"
---

# TASK-105 — Run custom SQL from a dedicated screen into a plain grid

Execution protocol, progress file grammar, and final report format are in `/task/AGENT.md`. Verbatim fixed-decision text is in `/task/decisions.md`. Both are read-only, as is this file.

## Goal

`x` in the Browser opens a SQL screen whose input, on `Enter`, runs exactly one statement and shows its rows in a non-sortable grid.

## Context

Current behavior:

- `Screen::CustomSql`, `App.sql_input`, `App.sql_grid` and `QueryKind::Custom` exist (TASK-101 enums) but `x` in the Browser is a no-op, nothing renders `CustomSql`, and `runtime::execute` only handles `QueryKind::Preview`.
- `PgSession::query` (`crates/pgtui/src/db/postgres.rs`) returns row results (TASK-104); it does not detect multiple row descriptions, does not report `CommandComplete` counts, and does not cap rows.
- `crates/pgtui/src/ui/custom_sql.rs` does not exist. Trusted test `app_custom_sql_test::navigation_x_opens_sql_screen` fails (`screen == Browser`).

Desired behavior:

- `x` → `Screen::CustomSql` (session and preview grid kept); `Esc` → `Screen::Browser` with `sql_input` and `sql_grid` retained.
- `Enter` sends the trimmed input minus one trailing `;` as `Effect::Query { kind: Custom, sql }`; empty input emits nothing.
- Rows → `sql_grid = Some(Grid)` (capped at 500 with `Status::Info("showing first 500 rows")`); `CommandComplete` → `Status::Info("ok: <n> rows affected")`, `sql_grid = None`; more than one row description → `DbError::MultiStatement` → `error: one statement at a time`.
- The result grid has no sort and no column cursor; `s`/`h`/`l` type into the input. Two protected snapshots match.

Read before editing (non-normative hints, in order):

1. `CLAUDE.md` — build, test, lint commands.
2. `/task/decisions.md` — D-010, D-011, D-013, D-025, D-026, D-034, D-041, D-050, D-060, D-071, D-072 verbatim; decided, do not reopen.
3. `crates/pgtui/src/app.rs`, `crates/pgtui/src/keys.rs` — Browser/grid handling from TASK-104; the CustomSql arm is new.
4. `crates/pgtui/src/db/postgres.rs`, `crates/pgtui/src/db/mod.rs` — `query` over `simple_query`; extend result handling and add `DbError::MultiStatement`.
5. `crates/pgtui/src/ui/grid.rs` — shared widget; add the plain mode (no `[ ]`, no sort markers).
6. `crates/pgtui/tests/{app_custom_sql,pg_custom_sql,screen_custom_sql}_test.rs` — the oracle.

Code flow: `keys::action_for` gains a `CustomSql` arm where every printable char is text input, so no letter commands exist on that screen. `App::update` owns `sql_input`/`sql_grid`; `runtime::execute(Effect::Query { Custom, .. })` calls `PgSession::query`, which walks the `SimpleQueryMessage` stream: one `RowDescription` + rows → `ResultSet`; `CommandComplete(n)` with no rows → rows-affected count; a second `RowDescription` → `DbError::MultiStatement`. `ui::draw` dispatches `Screen::CustomSql` to `ui/custom_sql.rs`, which stacks the input block above the results block and reuses `ui/grid.rs` in plain mode.

Baseline (run from repo root, before any edit):

```sh
cargo test -p pgtui --test app_custom_sql_test
```

Expected before this change: `navigation_x_opens_sql_screen` fails — assertion `screen == CustomSql` (actual `Browser`).

## Preconditions

Each precondition has a command. If a command fails, stop and report `BLOCKED` (see AGENT.md). Do not work around it.

- **P-001:** toolchain available — `cargo --version`
- **P-002:** protected test support present — `test -f crates/pgtui/tests/support/mod.rs`
- **P-003:** baseline tag resolvable — `git rev-parse --verify baseline`
- **P-004:** Docker reachable (testcontainers) — `docker info >/dev/null`

## Scope

In scope:

- CustomSql key mapping (D-034), `sql_input`/`sql_grid` state transitions, `x`/`Esc` navigation.
- `QueryKind::Custom` execution: trailing `;` strip, `MultiStatement`, `CommandComplete` info, 500-row cap with info status.
- `ui/custom_sql.rs`; plain mode flag on the grid widget.
- Executor unit test for `strip_trailing_semicolon` in `src/db/mod.rs`.

Out of scope:

- Query history, multi-line editor, autocomplete, cancelling a running query, disconnect (`d` stays a no-op in Browser), any change to `grid.rs` (frozen), keyring, dependency changes.

## Requirements

- **R-001 (MUST):** `x` in Browser → `Screen::CustomSql`, keeping `session` and `grid`; `Esc` → `Screen::Browser`, retaining `sql_input` and `sql_grid`.
- **R-002 (MUST):** Editing per D-034: printable chars append to `sql_input`, `Backspace` pops, `Enter` emits `Effect::Query { kind: Custom, sql }`; `Enter` on empty (after trim) input emits nothing.
- **R-003 (MUST):** SQL is sent verbatim after trimming whitespace and one trailing `;`. More than one row description → `DbError::MultiStatement` → `Status::Error("one statement at a time")` rendered `error: one statement at a time`. Non-row result → `Status::Info("ok: <n> rows affected")` and `sql_grid = None`. Row results capped client-side at `PREVIEW_LIMIT`; when capped, `Status::Info("showing first 500 rows")`.
- **R-004 (MUST):** The custom result grid has no sort and no column cursor: `s`, `h`, `l` are text input; `Up`/`Down` move the row cursor only; `sql_grid.sort` is always `None`.
- **R-005 (MUST):** Render per D-060 CustomSql (empty and with results), grid in plain mode.
- **R-006 (MUST):** Implement the final design directly. No compatibility layer, dual path, feature flag, or fallback.
- **R-007 (MUST NOT):** Add or upgrade dependencies; edit `Cargo.toml`, `Cargo.lock`, `justfile`, `rust-toolchain.toml`, `crates/pgtui/src/grid.rs`, or anything under `crates/pgtui/tests/`; use `#[ignore]`, `todo!`, `unimplemented!`, `dbg!`, or `#[allow(...)]`.

## Acceptance criteria

Each row is observable behavior with the exact evidence command. `verify.sh` runs these; the harness re-runs them.

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given Browser, when `x`, typing, `Backspace`, `Esc`, `x`, then screen transitions per D-034 and `sql_input` is retained. | `cargo test -p pgtui --test app_custom_sql_test navigation` | exit 0, `3 passed` |
| AC-002 | Given input `select 1 as a;`, when `Enter`, then `Effect::Query { Custom, "select 1 as a" }`; empty input emits nothing. | `cargo test -p pgtui --test app_custom_sql_test enter_` | exit 0, `2 passed` |
| AC-003 | Given the container, when `SELECT name FROM customers WHERE note IS NULL ORDER BY id` runs, then two rows; an `UPDATE` returns the rows-affected info; `select 1; select 2` → `MultiStatement`; a 600-row `generate_series` is capped at 500. | `cargo test -p pgtui --test pg_custom_sql_test` | exit 0, `4 passed` |
| AC-004 | Given a custom grid, when `s` is pressed, then `sql_input` ends with `s` and `sql_grid.sort` stays `None`. | `cargo test -p pgtui --test app_custom_sql_test no_sort_in_custom_grid` | exit 0, `1 passed` |
| AC-005 | Given the empty SQL screen and the screen after a fake result, when rendered, then `screen__custom_sql_empty` and `screen__custom_sql_results` (text + svg) match. | `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_custom_sql_test` | exit 0, `4 passed` |

## Fixed decisions

Already decided; full text in `/task/decisions.md`. Implement; do not reopen. Anything not listed that changes public behavior, architecture, data, or security posture is `NEEDS_REPLAN`, not executor discretion.

- **D-010, D-011:** `App` fields (`sql_input`, `sql_grid`, `status`), pure `update`, `Msg`/`Effect` shapes.
- **D-013:** status line semantics (`Help`/`Info`/`Error`, `error: <message>`).
- **D-025:** custom SQL semantics (trim, `;`, `MultiStatement`, `CommandComplete`, cap).
- **D-026:** `ResultSet`, `Cell`, `TableRef`.
- **D-034:** CustomSql keys (exact).
- **D-041:** every `Err` becomes `Status::Error`.
- **D-050:** `Grid` model — frozen; `Grid::from(ResultSet)` is reused unchanged for `sql_grid`.
- **D-060:** CustomSql layout and plain grid mode, to the character.
- **D-071, D-072:** snapshot policy and seed data.

## Checklist

Static plan. IDs `N`…`N.N.N.N`, max depth 4, four spaces per level. Every leaf names what becomes true and its evidence. State lives in `progress.md`, never here.

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-004` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test app_custom_sql_test` → `navigation_x_opens_sql_screen` fails (`screen == Browser`).
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Screen transitions and input editing (`R-001`, `R-002`, `D-034`) — evidence: `cargo test -p pgtui --test app_custom_sql_test navigation enter_` → `5 passed`.
    - [ ] **2.2** Custom query execution semantics (`R-003`, `D-025`) — evidence: `cargo test -p pgtui --test pg_custom_sql_test` → `4 passed`.
        - [ ] **2.2.1** Trailing `;` stripped; second row description → `DbError::MultiStatement` — evidence: `cargo test -p pgtui --test pg_custom_sql_test multi_statement_rejected trailing_semicolon` → `2 passed`.
        - [ ] **2.2.2** `CommandComplete` → info status and `sql_grid = None`; rows capped at 500 with info — evidence: `cargo test -p pgtui --test pg_custom_sql_test command_complete capped_` → `2 passed`.
    - [ ] **2.3** Custom grid is plain: no sort, no column cursor (`R-004`) — evidence: `cargo test -p pgtui --test app_custom_sql_test no_sort_in_custom_grid` → `1 passed`.
    - [ ] **2.4** CustomSql screen renders per `D-060` (`R-005`) — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_custom_sql_test` → `4 passed`.
    - [ ] **2.5** Unit test for `strip_trailing_semicolon` exists in `src/db/mod.rs` — evidence: `cargo test -p pgtui --lib db::tests::strip_trailing` → `1 passed` or more.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001`, `AC-002`, `AC-004` — evidence: `cargo test -p pgtui --test app_custom_sql_test` → `6 passed`.
    - [ ] **3.2** `AC-003` — evidence: `cargo test -p pgtui --test pg_custom_sql_test` → `4 passed`.
    - [ ] **3.3** `AC-005` — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_custom_sql_test` → `4 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated — evidence: `git status --porcelain` and `git diff --stat baseline` show only in-scope files.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full verifier output shown in the transcript.
<!-- checklist:end -->
