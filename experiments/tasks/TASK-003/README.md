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

Observable behaviour plus the exact evidence command. The gate runs these; the harness re-runs them.

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given the list screen, when `n` and then form keys are sent, then fields cycle with wrap, typing appends, backspace pops, port filters non-digits, and `Esc` discards. | `cargo test -p pgtui --test app_create_form_test` | exit 0, `9 passed` |
| AC-002 | Given a valid form, when `Enter` is pressed and the runtime runs, then a row is inserted and `Msg::Saved(Ok)` reloads the list with the new row selected; a duplicate name reports the decided error. | `cargo test -p pgtui --test runtime_create_test` | exit 0, `3 passed` |
| AC-003 | Given form state, when the 100x30 buffer is rendered, then labels, focus marker, masked password and help line appear as D-060 states. | `cargo test -p pgtui --test screen_create_form_test` | exit 0, `3 passed` |
| AC-004 | Given the whole task, when the earlier tasks' trusted tests run, then they still pass. | `cargo test -p pgtui --test store_test --test app_connection_list_test --test screen_connection_list_test --test cli_test --test app_create_form_test --test runtime_create_test --test screen_create_form_test` | exit 0, `31 passed` |
| AC-005 | Given the finished task, when the gate runs, then it reports `DONE`. | `taskfmt verify` | exit 0, last line `DONE` |

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
    - [ ] **2.2** D-032 key handling: cycling with wrap, typing, backspace, port filter, `Esc` (`R-002`, `AC-001`) — evidence: `cargo test -p pgtui --test app_create_form_test` prints `9 passed`.
- [ ] **3** Save flow is implemented.
    - [ ] **3.1** `Effect::SaveConnection` -> store insert -> `Msg::Saved` in `runtime.rs` (`R-003`, `AC-002`) — evidence: `cargo test -p pgtui --test runtime_create_test` prints `3 passed`.
    - [ ] **3.2** `Saved(Ok)` reloads the list with the new row selected; duplicate stays on the form (`R-003`, `R-004`) — evidence: `grep -q 'LoadConnections' crates/pgtui/src/app.rs && grep -q 'already exists' crates/pgtui/src/app.rs` exits 0.
- [ ] **4** Rendering is implemented.
    - [ ] **4.1** `ui/create_form.rs` per D-060 with masked password (`R-005`, `AC-003`) — evidence: `cargo test -p pgtui --test screen_create_form_test` prints `3 passed`.
    - [ ] **4.2** Lint is clean (`D-004`) — evidence: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] **5** Gate passes.
    - [ ] **5.1** Regression `AC-004` holds — evidence: the combined `cargo test -p pgtui --test ...` command of `AC-004` prints `31 passed`.
    - [ ] **5.2** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated (`R-006`, `R-007`) — evidence: `git status --porcelain` and `git diff --no-renames --stat $TASKFMT_BASE` show only in-scope files.
    - [ ] **5.3** `taskfmt verify` exits 0 with last line `DONE` (`AC-005`) — evidence: final full run (with progress check), full output shown in the transcript.
<!-- checklist:end -->
