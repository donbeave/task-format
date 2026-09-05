---
schema: task/v4
id: TASK-003
title: "Create-connection form and the save flow"
kind: feature
verify: "taskfmt verify"
expected_paths:
  - "Cargo.toml"
  - "Cargo.lock"
  - "crates/pgtui/Cargo.toml"
  - "crates/pgtui/src/app.rs"
  - "crates/pgtui/src/keys.rs"
  - "crates/pgtui/src/runtime.rs"
  - "crates/pgtui/src/ui/mod.rs"
  - "crates/pgtui/src/ui/create_form.rs"
---

# TASK-003 — Create-connection form and the save flow

Execution protocol, progress grammar, report format: `/task/AGENTS.md`. This file is read-only and states WHAT must become true.

## Goal

`n` opens the six-field create form, editing and validation behave as decided, and a valid form is saved through the store and appears selected in the connection list.

## Context

Current behavior:

- TASK-002 shipped the store, the connection list, the CLI loop and the status line; `n` switches to `Screen::CreateConnection` but no form exists, so nothing is rendered there and nothing can be saved.
- Trusted tests `app_create_form_test.rs`, `runtime_create_test.rs`, `screen_create_form_test.rs` are committed on the run base commit and fail to compile.

Desired behavior:

- `CreateForm`/`Field` exactly as D-010, form keys as D-032, validation messages as D-032, `Effect::SaveConnection` handled by `runtime::execute` against the store, and the D-060 CreateConnection layout; `Enter` on the list stays a no-op (TASK-004).

Read before editing (orientation only, non-normative, in order):

1. `crates/pgtui/tests/app_create_form_test.rs` — the state-machine oracle: field cycling, typing, port filtering, validation errors, `Esc`.
2. `crates/pgtui/tests/runtime_create_test.rs` and `crates/pgtui/tests/screen_create_form_test.rs` — persistence and rendering oracles.

Code flow: `keys.rs` routes printable chars, `Tab`/`BackTab`, `Backspace`, `Enter`, `Esc` while `screen == CreateConnection`; `App::update` mutates `self.form`, and on a valid form emits `Effect::SaveConnection(form.validate()?)`. `runtime::execute` calls `ConnectionStore::insert` and replies `Msg::Saved(..)`, which reloads the list (`Effect::LoadConnections`) or surfaces `error: name already exists`. `ui/create_form.rs` draws the six labelled lines with the `> ` focus marker and the masked password.

Baseline, from the repo root:

```sh
cargo test -p pgtui --test app_create_form_test
```

Expected before this change: a compile error — `CreateForm`/`Field` do not exist.

## Preconditions

If a command fails, stop and report `BLOCKED`. Never work around a precondition.

- **P-001:** toolchain available — `cargo --version`
- **P-002:** TASK-002 suites green — `cargo test -p pgtui --test store_test --test app_connection_list_test --test screen_connection_list_test --test cli_test`
- **P-003:** trusted form tests present — `test -f crates/pgtui/tests/app_create_form_test.rs`
- **P-004:** working tree clean — `test -z "$(git status --porcelain)"`

## Scope

In scope:

- `ui/create_form.rs`, `CreateForm`/`Field` in `app.rs`, D-032 key handling in `keys.rs`, `SaveConnection`/`Saved` in `runtime.rs`, form rendering in `ui/mod.rs`.

Out of scope:

- All of `db/` — the D-027 `db/mod.rs` placeholder TASK-002 left as well as `db/postgres.rs` — plus `browser.rs`, `custom_sql.rs`, `grid.rs`; later tasks.
- `render.rs`, `fonts/`, anything under `crates/pgtui/tests/`.
- Any change to the D-021/D-022 store contract.

## Requirements

- **R-001 (MUST):** `CreateForm` with `blank()` and `validate()` exactly as D-010; `Field` order Name, Host, Port, Database, User, Password.
- **R-002 (MUST):** Keys per D-032: `Tab`/`BackTab` cycle with wrap, printable chars append to the focused field, `Backspace` pops, port ignores non-digits, `Esc` discards and returns to the list.
- **R-003 (MUST):** Validation per D-032: first failing field as `error: <field> is required`, port as `error: port must be 1-65535`; success emits `Effect::SaveConnection`; duplicate name surfaces `error: name already exists` and stays on the form.
- **R-004 (MUST):** On `Msg::Saved(Ok)`: reload the list, return to `Screen::ConnectionList`, put the cursor on the new row (D-032, D-011).
- **R-005 (MUST):** Rendering per D-060: block title ` New connection `, lines `  <Label>: <value>`, `> ` on the focused line, password masked as `*` per char, help `Tab next  Enter save  Esc cancel`; the password never reaches the rendered buffer.
- **R-006 (MUST):** Final design directly; no compatibility layer, dual path, deprecated alias, feature flag, or legacy fallback.
- **R-007 (MUST NOT):** Remove or bump a D-001 pin, or depend on anything outside the D-001 pin list (adding a listed pin is required where trusted tests need it); touch `render.rs`, `fonts/`, or anything under `crates/pgtui/tests/`; change the store schema or API; wire `Enter` on the connection list; create or edit anything under `db/` (no `db/` path is in this task's scope whitelist); create `grid.rs`, `justfile`, or `README.md`.

## Acceptance criteria

Canonical typed acceptance blocks use taskfmt's Markdown profile. They are task metadata, not
Cucumber feature files and have no runtime step definitions.

### AC-001 — Form navigation wraps
Type: scenario
Class: delta
Covers: R-001, R-002
Evidence:
```sh
cargo test -p pgtui --test app_create_form_test
```
Expected: exit 0, `9 passed`

```gherkin
Given the connection list screen
When n opens the new-connection form and Tab or BackTab is pressed
Then the six fields cycle with wraparound and the blank form has the D-010 field order
```

### AC-002 — Form editing applies field rules
Type: scenario
Class: delta
Covers: R-002
Evidence:
```sh
cargo test -p pgtui --test app_create_form_test
```
Expected: exit 0, `9 passed`

```gherkin
Given a focused form field
When printable characters, Backspace, or a non-digit port character are entered
Then text appends, Backspace removes the last character, and the port keeps digits only
```

### AC-003 — Escape discards the form
Type: scenario
Class: invariant
Covers: R-002
Evidence:
```sh
cargo test -p pgtui --test app_create_form_test
```
Expected: exit 0, `9 passed`

```gherkin
Given a form containing unsaved values
When Esc is pressed
Then the values are discarded and the connection list returns
```

### AC-004 — A valid form is saved and selected
Type: scenario
Class: delta
Covers: R-003, R-004
Evidence:
```sh
cargo test -p pgtui --test runtime_create_test
```
Expected: exit 0, `3 passed`

```gherkin
Given a valid new-connection form
When Enter saves it through the runtime
Then the row is inserted, the list reloads, and the new row is selected
```

### AC-005 — Duplicate names stay on the form
Type: scenario
Class: failure
Covers: R-003
Evidence:
```sh
cargo test -p pgtui --test runtime_create_test
```
Expected: exit 0, `3 passed`

```gherkin
Given a connection name already exists
When a form with that name is saved
Then the decided duplicate-name error is shown and the form remains open
```

### AC-006 — Form rendering masks passwords
Type: scenario
Class: delta
Covers: R-005
Evidence:
```sh
cargo test -p pgtui --test screen_create_form_test
```
Expected: exit 0, `3 passed`

```gherkin
Given a populated form with one focused field
When the 100x30 form buffer is rendered
Then labels, focus, masked password characters, and the D-060 help line appear
```

### AC-007 — Earlier trusted behavior remains green
Type: invariant
Class: regression
Covers: R-001, R-002, R-003, R-004, R-005
Evidence:
```sh
cargo test -p pgtui \
  --test store_test \
  --test app_connection_list_test \
  --test screen_connection_list_test \
  --test cli_test \
  --test app_create_form_test \
  --test runtime_create_test \
  --test screen_create_form_test \
  --test skeleton_test -- --skip pgtui_stub_exits_2
```
Expected: exit 0, 8 `test result: ok.` lines (one per target), no `FAILED`

```gherkin
The complete trusted suite for the task remains green.
```

### AC-008 — Completion gate passes
Type: gate
Evidence:
```sh
taskfmt verify
```
Expected: exit 0, last line `DONE`

## Fixed decisions

Full text: `/task/decisions.md` (binding, read-only).
Implement; do not reopen. Anything not decided there that changes public behaviour, architecture, data or security posture is `NEEDS_REPLAN`, not executor discretion.

- **D-010:** `CreateForm`/`Field` shape and `blank()`/`validate()`.
- **D-011, D-012:** `SaveConnection`/`Saved` round trip through the async runtime.
- **D-032:** form keys, validation messages, save and cancel transitions.
- **D-060:** CreateConnection layout; D-061 single `ui::draw` entry point.
- **D-041:** errors surface as status, never swallowed.
- **D-070, D-071:** trusted helpers and behavioural verification.

## Checklist

Static plan. Hierarchical IDs, four spaces per level, max depth 4. Every leaf names what becomes true and the evidence permitting the check.

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-004` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test app_create_form_test` fails with `CreateForm`/`Field` unresolved.
- [ ] **2** Form state is implemented.
    - [ ] **2.1** `CreateForm`/`Field` per D-010 with `blank()` and `validate()` (`R-001`) — evidence: `grep -q 'pub enum Field' crates/pgtui/src/app.rs && grep -q 'pub fn validate' crates/pgtui/src/app.rs` exits 0.
    - [ ] **2.2** Form navigation wraps (`R-002`, `AC-001`) — evidence: `cargo test -p pgtui --test app_create_form_test` prints `9 passed` for navigation.
- [ ] **3** Form editing and cancellation are implemented.
    - [ ] **3.1** Form editing applies field rules (`R-002`, `AC-002`) — evidence: `cargo test -p pgtui --test app_create_form_test` prints `9 passed` for editing.
    - [ ] **3.2** Escape discards the form (`R-002`, `AC-003`) — evidence: `cargo test -p pgtui --test app_create_form_test` prints `9 passed` for cancellation.
- [ ] **4** Save flow is implemented.
    - [ ] **4.1** A valid form saves and selects the new row (`R-003`, `R-004`, `AC-004`) — evidence: `cargo test -p pgtui --test runtime_create_test` prints `3 passed` for the success path.
    - [ ] **4.2** Duplicate names keep the form open (`R-003`, `AC-005`) — evidence: `cargo test -p pgtui --test runtime_create_test` prints `3 passed` for duplicate handling.
- [ ] **5** Rendering and verification are complete.
    - [ ] **5.1** Form rendering and lint are complete.
        - [ ] **5.1.1** Form rendering masks passwords (`R-005`, `AC-006`) — evidence: `cargo test -p pgtui --test screen_create_form_test` prints `3 passed`.
        - [ ] **5.1.2** Lint is clean (`D-004`) — evidence: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` exits 0.
    - [ ] **5.2** Regression `AC-007` holds — evidence: the `AC-007` command exits 0 and prints 8 `test result: ok.` lines, one per `--test` target, and no `FAILED`.
    - [ ] **5.3** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated (`R-006`, `R-007`) — evidence: `git status --porcelain` and `git diff --no-renames --stat $TASKFMT_BASE` show only in-scope files.
    - [ ] **5.4** `taskfmt verify` exits 0 with last line `DONE` (`AC-008`) — evidence: final full run (with progress check), full output shown in the transcript.
<!-- checklist:end -->
