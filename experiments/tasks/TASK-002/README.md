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
- **P-002:** TASK-001 scaffold builds — `cargo build --workspace --lib --bins`
- **P-003:** trusted store tests present — `test -f crates/pgtui/tests/store_test.rs`
- **P-004:** working tree clean — `test -z "$(git status --porcelain)"`

## Scope

In scope:

- `app.rs`, `keys.rs`, `runtime.rs`, `store/mod.rs`, `db/mod.rs` (D-027 placeholders only), `ui/mod.rs`, `ui/connection_list.rs`, `ui/status.rs`.
- `lib.rs` module declarations, `main.rs` CLI and loop.

Out of scope:

- `create_form.rs`, `browser.rs`, `custom_sql.rs`, `grid.rs`, `db/postgres.rs`; later tasks.
- `render.rs`, `fonts/`, anything under `crates/pgtui/tests/`.
- `README.md`, keyring, any repository task runner (D-005).

## Requirements

- **R-001 (MUST):** Store API exactly as D-022, schema exactly as D-021, path precedence exactly as D-020; `list()` is `ORDER BY name ASC`; `insert` on a duplicate name returns `StoreError::DuplicateName`.
- **R-002 (MUST):** `App`, `Screen`, `Msg`, `Effect`, `QueryKind`, `QueryOutcome` with the exact variant sets of D-010/D-011; types whose task has not landed yet are the D-027 placeholders in `db/mod.rs`, but the field set of `App` is final.
- **R-003 (MUST):** Keys per D-030/D-031: `j`/`k`/`Up`/`Down` clamped, `n` opens a blank form screen, `q` and `Ctrl+C` quit; `Enter` stays a no-op until TASK-004.
- **R-004 (MUST):** Rendering per D-060/D-061: 100x30 buffer, list block ` pgtui - connections `, rows `<name>  <display_dsn>`, highlight `> `, empty-list hint, help line, status line per D-013.
- **R-005 (MUST):** `main.rs` per D-012/D-040/D-042: `--db <path>`, store open failure exits 2 with `error: ...` on stderr, `q`/`Ctrl+C` exits 0 after restoring the terminal; the stub behaviour (`error: not implemented`, exit 2) is gone, and a non-interactive invocation follows D-042 instead.
- **R-006 (MUST):** Final design directly; no compatibility layer, dual path, deprecated alias, feature flag, or legacy fallback.
- **R-007 (MUST NOT):** Remove or bump a D-001 pin, or depend on anything outside the D-001 pin list (adding a listed pin is required where trusted tests need it); touch `render.rs`, `fonts/`, or anything under `crates/pgtui/tests/`; introduce `tokio_postgres` in `crates/pgtui/src/`; create `db/postgres.rs`, `grid.rs`, `justfile`, or `README.md`.

## Acceptance criteria

Canonical typed acceptance blocks use taskfmt's Markdown profile. They are task metadata, not
Cucumber feature files and have no runtime step definitions.

### AC-001 — Connections persist and sort
Type: scenario
Class: delta
Covers: R-001
Evidence: `cargo test -p pgtui --test store_test`
Expected: exit 0, `5 passed`

```gherkin
Given a fresh temporary database
When named connections are inserted and the store is reopened
Then connections are listed in ascending name order and survive the reopen
```

### AC-002 — Duplicate names are rejected
Type: scenario
Class: delta
Covers: R-001
Evidence: `cargo test -p pgtui --test store_test`
Expected: exit 0, `5 passed`

```gherkin
Given a connection name already exists
When the same name is inserted again
Then the store returns the decided duplicate-name error
```

### AC-003 — List navigation is clamped
Type: scenario
Class: delta
Covers: R-002, R-003
Evidence: `cargo test -p pgtui --test app_connection_list_test`
Expected: exit 0, `5 passed`

```gherkin
Given an App with saved connections
When navigation keys move beyond either end of the list
Then the cursor remains clamped within the list
```

### AC-004 — List quit keys emit the quit effect
Type: scenario
Class: invariant
Covers: R-003
Evidence: `cargo test -p pgtui --test app_connection_list_test`
Expected: exit 0, `5 passed`

```gherkin
Given the connection-list screen
When q or Ctrl+C is pressed
Then App emits Effect::Quit
```

### AC-005 — Connection rows render without secrets
Type: scenario
Class: delta
Covers: R-004
Evidence: `cargo test -p pgtui --test screen_connection_list_test`
Expected: exit 0, `3 passed`

```gherkin
Given saved connections and a selected row
When the 100x30 connection-list buffer is rendered
Then the title, DSN rows, and selected-row marker match D-060 without a password
```

### AC-006 — Empty-list guidance and status render
Type: scenario
Class: invariant
Covers: R-004
Evidence: `cargo test -p pgtui --test screen_connection_list_test`
Expected: exit 0, `3 passed`

```gherkin
Given an empty connection list
When the connection-list buffer is rendered
Then its hint, help line, and status line are visible
```

### AC-007 — CLI diagnostics follow D-040
Type: scenario
Class: delta
Covers: R-005
Evidence: `cargo test -p pgtui --test cli_test`
Expected: exit 0, `3 passed`

```gherkin
Given an invalid store path or a version/help request
When the pgtui command is invoked
Then the decided stderr diagnostics and exit codes are returned
```

### AC-008 — Interactive quit restores the terminal
Type: scenario
Class: invariant
Covers: R-005
Evidence: `cargo test -p pgtui --test cli_test`
Expected: exit 0, `3 passed`

```gherkin
Given the real pgtui binary in an interactive terminal
When a quit key ends the connection-list session
Then the process returns successfully after restoring the terminal
```

### AC-009 — Earlier trusted behavior remains green
Type: invariant
Class: regression
Covers: R-001, R-002, R-003, R-004, R-005
Evidence: `cargo test -p pgtui --test store_test --test app_connection_list_test --test screen_connection_list_test --test cli_test --test skeleton_test -- --skip pgtui_stub_exits_2`
Expected: exit 0, 5 `test result: ok.` lines (one per target), no `FAILED`

```gherkin
The complete trusted suite for the task remains green.
```

### AC-010 — Completion gate passes
Type: gate
Evidence: `taskfmt verify`
Expected: exit 0, last line `DONE`

## Fixed decisions

Full text: `/task/decisions.md` (binding, read-only).
Implement; do not reopen. Anything not decided there that changes public behaviour, architecture, data or security posture is `NEEDS_REPLAN`, not executor discretion.

- **D-010, D-011:** `App` field set, `Screen`/`Msg`/`Effect`/`QueryKind`/`QueryOutcome` variants, pure `update` plus async `runtime::execute`.
- **D-012, D-013:** runtime loop shape; status line semantics.
- **D-020..D-022, D-027:** Turso store, schema, exact API; the `db/mod.rs` placeholder.
- **D-030, D-031:** global and ConnectionList keys.
- **D-040..D-042:** exit codes; error surfacing; the non-interactive contract.
- **D-060, D-061:** layout and the single `ui::draw` entry point.
- **D-070, D-071:** trusted helpers and behavioural verification.

## Checklist

Static plan. Hierarchical IDs, four spaces per level, max depth 4. Every leaf names what becomes true and the evidence permitting the check.

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-004` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test store_test` fails with `could not find pgtui::store`.
- [ ] **2** Store is implemented.
    - [ ] **2.1** Connections persist and sort (`R-001`, `AC-001`) — evidence: `cargo test -p pgtui --test store_test` prints `5 passed` for persistence and ordering.
    - [ ] **2.2** Duplicate names are rejected (`R-001`, `AC-002`) — evidence: `grep -q 'DuplicateName' crates/pgtui/src/store/mod.rs && grep -q 'CREATE TABLE IF NOT EXISTS connections' crates/pgtui/src/store/mod.rs` exits 0.
- [ ] **3** State machine and keys are implemented.
    - [ ] **3.1** `app.rs` carries the D-010 field set and list navigation clamps (`R-002`, `R-003`, `AC-003`) — evidence: `cargo test -p pgtui --test app_connection_list_test` prints `5 passed` for cursor behavior.
    - [ ] **3.2** List quit keys emit `Effect::Quit` (`R-003`, `AC-004`) — evidence: `cargo test -p pgtui --test app_connection_list_test` prints `5 passed` for quit behavior.
- [ ] **4** Rendering and process behaviour are implemented.
    - [ ] **4.1** Connection rows render without secrets (`R-004`, `AC-005`) — evidence: `cargo test -p pgtui --test screen_connection_list_test` prints `3 passed` for populated rows.
    - [ ] **4.2** CLI behavior is complete (`R-005`).
        - [ ] **4.2.1** `main.rs` reports CLI diagnostics (`AC-007`) — evidence: `cargo test -p pgtui --test cli_test` prints `3 passed` for diagnostics.
        - [ ] **4.2.2** Interactive quit restores the terminal (`AC-008`) — evidence: `cargo test -p pgtui --test cli_test` prints `3 passed` for interactive quit.
    - [ ] **4.3** Rendering contract is complete.
        - [ ] **4.3.1** Empty-list guidance and status render (`R-004`, `AC-006`) — evidence: `cargo test -p pgtui --test screen_connection_list_test` prints `3 passed` for the empty state.
        - [ ] **4.3.2** Lint is clean (`D-004`) — evidence: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] **5** Gate passes.
    - [ ] **5.1** Regression `AC-009` holds — evidence: the `AC-009` command exits 0 and prints 5 `test result: ok.` lines, one per `--test` target, and no `FAILED`.
    - [ ] **5.2** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated (`R-006`, `R-007`) — evidence: `git status --porcelain` and `git diff --no-renames --stat $TASKFMT_BASE` show only in-scope files.
    - [ ] **5.3** `taskfmt verify` exits 0 with last line `DONE` (`AC-010`) — evidence: final full run (with progress check), full output shown in the transcript.
<!-- checklist:end -->
