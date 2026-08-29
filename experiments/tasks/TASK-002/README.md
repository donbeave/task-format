---
schema: task/v4
id: TASK-002
title: "Connection store, connection list screen and CLI skeleton"
kind: feature
verify: "taskfmt verify"
expected_paths:
  - "Cargo.toml"
  - "Cargo.lock"
  - "crates/pgtui/Cargo.toml"
  - "crates/pgtui/src/lib.rs"
  - "crates/pgtui/src/main.rs"
  - "crates/pgtui/src/app.rs"
  - "crates/pgtui/src/keys.rs"
  - "crates/pgtui/src/runtime.rs"
  - "crates/pgtui/src/store/mod.rs"
  - "crates/pgtui/src/db/mod.rs"
  - "crates/pgtui/src/ui/mod.rs"
  - "crates/pgtui/src/ui/connection_list.rs"
  - "crates/pgtui/src/ui/status.rs"
---

# TASK-002 — Connection store, connection list screen and CLI skeleton

Execution protocol, progress grammar, report format: `/task/AGENTS.md`. This file is read-only and states WHAT must become true.

## Goal

The app persists connections in a local Turso file, renders the sorted connection list with a cursor, starts from the CLI with `--db`, and reaches exit code 0 on `q`/`Ctrl+C` from the list.

## Context

Current behavior:

- TASK-001 left a workspace whose only modules are the planner-shipped `render` and two exit-2 stubs; there is no store, no `App`, no UI.
- Trusted tests `store_test.rs`, `app_connection_list_test.rs`, `screen_connection_list_test.rs`, `cli_test.rs` are already committed on the run base commit and fail to compile.

Desired behavior:

- `crates/pgtui/src/store/mod.rs` implements D-020..D-022; `app.rs` implements D-010/D-011 with the full `Screen`, `Msg` and `Effect` sets; `keys.rs` maps D-030/D-031; `ui/` renders the list and the status line; `main.rs` runs the D-012 loop for the connection list; not-yet-implemented behaviour keeps failing with exit 2 or is a no-op exactly as D-031 states.

Read before editing (orientation only, non-normative, in order):

1. `crates/pgtui/tests/store_test.rs` and `crates/pgtui/tests/app_connection_list_test.rs` — the store and state-machine oracles.
2. `crates/pgtui/tests/screen_connection_list_test.rs` and `crates/pgtui/tests/cli_test.rs` — the rendering and process-level oracles.

Code flow: `main.rs` parses `--db`, opens the store, feeds `Msg::Connections(list)` into `App::update`, then loops draw/read-key/effects. `keys.rs` turns a `KeyEvent` into the `Msg::Key` handling that `App::update` performs; effects that need IO go through `runtime::execute` (D-011). Rendering goes `ui::draw(app, &mut Buffer)` (D-061), split across `ui/connection_list.rs` and `ui/status.rs`.

Baseline, from the repo root:

```sh
cargo test -p pgtui --test store_test
```

Expected before this change: a compile error — `could not find pgtui::store`.

## Preconditions

If a command fails, stop and report `BLOCKED`. Never work around a precondition.

- **P-001:** toolchain available — `cargo --version`
- **P-002:** TASK-001 gate green — `taskfmt verify`
- **P-003:** trusted store tests present — `test -f crates/pgtui/tests/store_test.rs`
- **P-004:** working tree clean — `test -z "$(git status --porcelain)"`

## Scope

In scope:

- `app.rs`, `keys.rs`, `runtime.rs`, `store/mod.rs`, `db/mod.rs` (D-002 placeholders only), `ui/mod.rs`, `ui/connection_list.rs`, `ui/status.rs`.
- `lib.rs` module declarations, `main.rs` CLI and loop.

Out of scope:

- `create_form.rs`, `browser.rs`, `custom_sql.rs`, `grid.rs`, `db/postgres.rs`; later tasks.
- `render.rs`, `fonts/`, anything under `crates/pgtui/tests/`.
- `README.md`, keyring, any repository task runner (D-005).

## Requirements

- **R-001 (MUST):** Store API exactly as D-022, schema exactly as D-021, path precedence exactly as D-020; `list()` is `ORDER BY name ASC`; `insert` on a duplicate name returns `StoreError::DuplicateName`.
- **R-002 (MUST):** `App`, `Screen`, `Msg`, `Effect`, `QueryKind`, `QueryOutcome` with the exact variant sets of D-010/D-011; types whose task has not landed yet may be empty placeholders in `db/mod.rs` (D-002), but the field set of `App` is final.
- **R-003 (MUST):** Keys per D-030/D-031: `j`/`k`/`Up`/`Down` clamped, `n` opens a blank form screen, `q` and `Ctrl+C` quit; `Enter` stays a no-op until TASK-004.
- **R-004 (MUST):** Rendering per D-060/D-061: 100x30 buffer, list block ` pgtui - connections `, rows `<name>  <display_dsn>`, highlight `> `, empty-list hint, help line, status line per D-013.
- **R-005 (MUST):** `main.rs` per D-012/D-040: `--db <path>`, store open failure exits 2 with `error: ...` on stderr, `q`/`Ctrl+C` exits 0 after restoring the terminal; stub behaviour (`error: not implemented`, exit 2) is gone.
- **R-006 (MUST):** Final design directly; no compatibility layer, dual path, deprecated alias, feature flag, or legacy fallback.
- **R-007 (MUST NOT):** Remove or bump a D-001 pin, or depend on anything outside the D-001 pin list (adding a listed pin is required where trusted tests need it); touch `render.rs`, `fonts/`, or anything under `crates/pgtui/tests/`; introduce `tokio_postgres` in `crates/pgtui/src/`; create `db/postgres.rs`, `grid.rs`, `justfile`, or `README.md`.

## Acceptance criteria

Observable behaviour plus the exact evidence command. The gate runs these; the harness re-runs them.

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given a fresh temp database, when connections are inserted, then they are listed sorted by name, survive a reopen, and a duplicate name is rejected. | `cargo test -p pgtui --test store_test` | exit 0, `5 passed` |
| AC-002 | Given an `App` with saved connections, when key messages arrive, then the cursor moves clamped and `q`/`Ctrl+C` emit `Effect::Quit`. | `cargo test -p pgtui --test app_connection_list_test` | exit 0, `5 passed` |
| AC-003 | Given that state, when the 100x30 buffer is rendered, then title, hint, help, DSN rows and the `> ` cursor appear as D-060 states, with no password on screen. | `cargo test -p pgtui --test screen_connection_list_test` | exit 0, `3 passed` |
| AC-004 | Given the real binary, when the store path is unwritable or `--version`/`--help` is used, then the D-040 exit codes and stderr messages appear. | `cargo test -p pgtui --test cli_test` | exit 0, `3 passed` |
| AC-005 | Given the whole task, when the earlier tasks' trusted tests run, then they still pass. | `cargo test -p pgtui --test skeleton_test --test store_test --test app_connection_list_test --test screen_connection_list_test --test cli_test` | exit 0, `20 passed` |
| AC-006 | Given the finished task, when the gate runs, then it reports `DONE`. | `taskfmt verify` | exit 0, last line `DONE` |

## Fixed decisions

Full text: `/task/decisions.md` (binding, read-only).
Implement; do not reopen. Anything not decided there that changes public behaviour, architecture, data or security posture is `NEEDS_REPLAN`, not executor discretion.

- **D-010, D-011:** `App` field set, `Screen`/`Msg`/`Effect`/`QueryKind`/`QueryOutcome` variants, pure `update` plus async `runtime::execute`.
- **D-012, D-013:** runtime loop shape; status line semantics.
- **D-020..D-022:** Turso store, schema, exact API.
- **D-030, D-031:** global and ConnectionList keys.
- **D-040, D-041:** exit codes; error surfacing.
- **D-060, D-061:** layout and the single `ui::draw` entry point.
- **D-070, D-071:** trusted helpers and behavioural verification.

## Checklist

Static plan. Hierarchical IDs, four spaces per level, max depth 4. Every leaf names what becomes true and the evidence permitting the check.

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-004` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test store_test` fails with `could not find pgtui::store`.
- [ ] **2** Store is implemented.
    - [ ] **2.1** `store/mod.rs` matches D-020..D-022 (`R-001`, `AC-001`) — evidence: `cargo test -p pgtui --test store_test` prints `5 passed`.
    - [ ] **2.2** Schema, path precedence and duplicate handling match D-020/D-021 (`R-001`) — evidence: `grep -q 'DuplicateName' crates/pgtui/src/store/mod.rs && grep -q 'CREATE TABLE IF NOT EXISTS connections' crates/pgtui/src/store/mod.rs` exits 0.
- [ ] **3** State machine and keys are implemented.
    - [ ] **3.1** `app.rs` carries the D-010 field set and D-011 variant sets (`R-002`, `AC-002`) — evidence: `cargo test -p pgtui --test app_connection_list_test` prints `5 passed`.
    - [ ] **3.2** `keys.rs` maps D-030/D-031, `Enter` still inert (`R-003`) — evidence: `grep -q 'Effect::Quit' crates/pgtui/src/keys.rs && grep -q 'KeyCode::Up' crates/pgtui/src/keys.rs` exits 0.
- [ ] **4** Rendering and process behaviour are implemented.
    - [ ] **4.1** `ui/connection_list.rs` and `ui/status.rs` satisfy D-060/D-013 (`R-004`, `AC-003`) — evidence: `cargo test -p pgtui --test screen_connection_list_test` prints `3 passed`.
    - [ ] **4.2** `main.rs` runs the D-012 loop with D-040 exit codes (`R-005`, `AC-004`) — evidence: `cargo test -p pgtui --test cli_test` prints `3 passed`.
    - [ ] **4.3** Lint is clean (`D-004`) — evidence: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] **5** Gate passes.
    - [ ] **5.1** Regression `AC-005` holds — evidence: `cargo test -p pgtui --test skeleton_test --test store_test --test app_connection_list_test --test screen_connection_list_test --test cli_test` prints `20 passed`.
    - [ ] **5.2** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated (`R-006`, `R-007`) — evidence: `git status --porcelain` and `git diff --no-renames --stat $TASKFMT_BASE` show only in-scope files.
    - [ ] **5.3** `taskfmt verify` exits 0 with last line `DONE` (`AC-006`) — evidence: final full run (with progress check), full output shown in the transcript.
<!-- checklist:end -->
