---
schema: task/v3
id: TASK-102
title: "Create a new connection from the TUI and show it selected in the list"
kind: feature
verify: "/task/verify.sh"
expected_paths:
  - "crates/pgtui/src/app.rs"
  - "crates/pgtui/src/keys.rs"
  - "crates/pgtui/src/runtime.rs"
  - "crates/pgtui/src/ui/create_form.rs"
  - "crates/pgtui/src/ui/mod.rs"
---

# TASK-102 — Create a new connection from the TUI and show it selected in the list

Execution protocol, progress file grammar, and final report format are in `/task/AGENTS.md`. This file is read-only. It defines WHAT must become true; it does not change during execution.

## Goal

Pressing `n` on the connection list opens a six-field form whose valid submission persists a connection and returns to the list with the new row selected.

## Context

Current behavior:

- `pgtui` (TASK-101 result) lists saved connections; `keys.rs` maps `n` to nothing, `App::form` is a placeholder `CreateForm`, `ui/create_form.rs` does not exist, and `runtime::execute` handles only `LoadConnections`/`Quit`.
- `crates/pgtui/tests/app_create_form_test.rs` does not compile (`CreateForm` fields/`Field` enum missing).

Desired behavior:

- `n` → `Screen::CreateConnection` with a blank form focused on Name; `Tab`/`BackTab` cycle Name, Host, Port, Database, User, Password; `Enter` validates and emits `Effect::SaveConnection`; on `Msg::Saved(Ok)` the list reloads, returns to `ConnectionList`, cursor on the new row; `DuplicateName` → `error: name already exists`, form kept; `Esc` discards.

Read before editing (non-normative hints, in order):

1. `AGENTS.md` — build/test/lint commands.
2. `/task/decisions.md` — verbatim D-* text (form keys D-032, validation strings, layout D-060 CreateConnection). Decided; do not reopen.
3. `crates/pgtui/src/app.rs`, `keys.rs`, `runtime.rs` — existing `Msg`/`Effect`/`Status` handling to extend.
4. `crates/pgtui/src/store/mod.rs` — frozen `ConnectionStore::insert` + `StoreError::DuplicateName` (protected; use, do not edit).
5. `crates/pgtui/tests/app_create_form_test.rs`, `runtime_create_test.rs`, `screen_create_form_test.rs` — the oracles; they show the exact `CreateForm { fields, focus }` / `Field` surface.

Code flow: `keys::action_for` maps keys per screen; on `CreateConnection` printable chars and `Backspace` edit the focused field, `Tab`/`BackTab` move focus, `Enter` calls `CreateForm::validate() -> Result<NewConnection, String>`; `Ok` → `Effect::SaveConnection(new)`, `Err(msg)` → `Status::Error(msg)`. `runtime::execute(SaveConnection)` calls `store.insert` and replies `Msg::Saved(result)`; `update(Saved(Ok(row)))` sets `pending_select = Some(row.name)`, emits `Effect::LoadConnections`; `update(Connections(list))` then places the cursor on that name. `ui::draw` dispatches to `ui::create_form::draw` when `screen == CreateConnection`.

Baseline (run from repo root, before any edit):

```sh
cargo test -p pgtui --test app_create_form_test
```

Expected before this change: compile error (`CreateForm` has no fields; `pgtui::app::Field` unresolved).

## Preconditions

Each precondition has a command. If a command fails, stop and report `BLOCKED` (see AGENTS.md). Do not work around it.

- **P-001:** toolchain available — `cargo --version`
- **P-002:** test support present — `test -f crates/pgtui/tests/support/mod.rs`
- **P-003:** baseline tag resolvable — `git rev-parse --verify baseline`

## Scope

In scope:

- `CreateForm` + `Field` in `app.rs`; `keys.rs` CreateConnection mapping (D-032); validation; `Msg::Saved` handling and cursor placement.
- `runtime.rs` execution of `Effect::SaveConnection`.
- `ui/create_form.rs` and its dispatch in `ui/mod.rs`.
- Executor unit tests for `CreateForm::validate` in `src/app.rs`.

Out of scope:

- Editing/deleting connections, connecting (`Enter` on the list stays a no-op), password masking toggle, keyring.
- Any change to `store/` (frozen), dependency changes, formatting sweeps.

## Requirements

- **R-001 (MUST):** Form fields, focus cycling (wrap both directions), char append, `Backspace` pop, Port digits-only — exactly per D-032.
- **R-002 (MUST):** `Enter` validates per D-032: messages `<field> is required` / `port must be 1-65535`, first failing field in order; on failure `Status::Error` and screen stays `CreateConnection`.
- **R-003 (MUST):** Valid `Enter` → `Effect::SaveConnection`; `Msg::Saved(Ok)` → `Effect::LoadConnections`, `Screen::ConnectionList`, cursor on the new row; `Msg::Saved(Err(DuplicateName))` → `Status::Error("name already exists")`, form kept.
- **R-004 (MUST):** `Esc` → `Screen::ConnectionList`, form discarded (next `n` shows a blank form).
- **R-005 (MUST):** Render per D-060 CreateConnection: `> ` focus prefix, `*` per password char, help text.
- **R-006 (MUST):** Implement the final design directly. No compatibility layer, feature flag, or fallback.
- **R-007 (MUST NOT):** Add/upgrade dependencies or edit `Cargo.toml`, `Cargo.lock`, `justfile`, `rust-toolchain.toml`, `store/`, `render.rs`, or anything under `tests/`. Executor tests live in `src/` only (D-003).

## Acceptance criteria

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given the list, when `n` then typing into fields with `Tab`/`BackTab`, then form state matches the typed values and Port ignores non-digits. | `cargo test -p pgtui --test app_create_form_test editing` | exit 0, `4 passed` |
| AC-002 | Given a form with empty Name, empty Host, or Port `0`, when `Enter`, then `Status::Error` names the field and screen stays `CreateConnection`. | `cargo test -p pgtui --test app_create_form_test validation` | exit 0, `3 passed` |
| AC-003 | Given a valid form, when `Enter` and the runtime executes the effect against a temp store, then the store has the row and the app is on `ConnectionList` with the cursor on it. | `cargo test -p pgtui --test runtime_create_test save_roundtrip` | exit 0, `1 passed` |
| AC-004 | Given an existing name, when submitted again, then `error: name already exists` and the form is kept. | `cargo test -p pgtui --test runtime_create_test duplicate_keeps_form` | exit 0, `1 passed` |
| AC-005 | Given a blank form and a filled form with Password focused, when rendered, then `screen__create_form_blank` and `screen__create_form_filled` (text + svg) match. | `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_create_form_test -- screen__create_form_blank screen__create_form_filled` | exit 0, `4 passed` |
| AC-006 | Given a saved form, when rendered after `Msg::Saved(Ok)` and the reloaded list, then `screen__create_form_saved_list` matches with the new row selected. | `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_create_form_test screen__create_form_saved_list` | exit 0, `2 passed` |

## Fixed decisions

Already decided. Implement; do not reopen. Verbatim text in `/task/decisions.md`. Anything not listed there that changes public behavior, architecture, data, or security posture is `NEEDS_REPLAN`.

- **D-010..D-013:** `Screen`/`App` fields, `Msg`/`Effect`, pure `update`, runtime loop, status line.
- **D-021, D-022:** schema and frozen store API (`insert`, `DuplicateName`).
- **D-032:** CreateConnection keys, field order, validation messages, save/duplicate/Esc semantics.
- **D-041:** error surfacing.
- **D-060:** CreateConnection layout strings; **D-071:** snapshot policy and names.

## Checklist

Static plan. IDs `N`…`N.N.N.N`, max depth 4, four spaces per level. State lives in `progress.md`.

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-003` pass — evidence: each command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test app_create_form_test` fails to compile (`CreateForm`/`Field` missing).
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Form state and editing keys (`R-001`, `D-032`) — evidence: `cargo test -p pgtui --test app_create_form_test editing` → `4 passed`.
        - [ ] **2.1.1** `n` enters `Screen::CreateConnection` with a blank form focused on Name — evidence: `cargo test -p pgtui --test app_create_form_test editing_n_opens_blank_form` → `1 passed`.
        - [ ] **2.1.2** `Tab`/`BackTab` wrap through the six fields; chars append, `Backspace` pops, Port digits only — evidence: `cargo test -p pgtui --test app_create_form_test -- editing_tab editing_chars editing_port` → `3 passed`.
    - [ ] **2.2** Validation and error messages (`R-002`, `D-032`, `D-013`) — evidence: `cargo test -p pgtui --test app_create_form_test validation` → `3 passed`.
    - [ ] **2.3** Save effect, reload, cursor placement, duplicate handling, `Esc` (`R-003`, `R-004`) — evidence: `cargo test -p pgtui --test runtime_create_test` → `3 passed`.
        - [ ] **2.3.1** `runtime::execute(SaveConnection)` inserts and replies `Msg::Saved`; `Saved(Ok)` reloads and selects the row — evidence: `cargo test -p pgtui --test runtime_create_test save_roundtrip` → `1 passed`.
        - [ ] **2.3.2** `Saved(Err(DuplicateName))` keeps the form with the error status; `Esc` discards — evidence: `cargo test -p pgtui --test runtime_create_test -- duplicate_keeps_form esc_discards` → `2 passed`.
    - [ ] **2.4** Form renders per `D-060` with masked password (`R-005`) — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_create_form_test` → `6 passed`.
    - [ ] **2.5** Unit tests for `CreateForm::validate` exist in `src/app.rs` — evidence: `cargo test -p pgtui --lib -- --list | grep -c 'app::tests::validate'` prints ≥ 3.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001`, `AC-002` — evidence: `cargo test -p pgtui --test app_create_form_test` → `7 passed`.
    - [ ] **3.2** `AC-003`, `AC-004` — evidence: `cargo test -p pgtui --test runtime_create_test` → `3 passed`.
    - [ ] **3.3** `AC-005`, `AC-006` — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_create_form_test` → `6 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only the five `expected_paths` files changed, nothing temporary or unrelated — evidence: `git status --porcelain` shows only those paths.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full verifier output shown in the transcript.
<!-- checklist:end -->
