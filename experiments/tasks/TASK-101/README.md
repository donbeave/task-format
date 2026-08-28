---
schema: task/v3
id: TASK-101
title: "List saved PostgreSQL connections from a Turso store in a ratatui shell"
kind: feature
verify: "/task/verify.sh"
expected_paths:
  - "crates/pgtui/src/*"
protected_paths:
  - "crates/pgtui/tests/*"
  - "crates/pgtui/src/render.rs"
  - "crates/pgtui/src/fonts/*"
  - "Cargo.toml"
  - "Cargo.lock"
  - "crates/pgtui/Cargo.toml"
  - "justfile"
  - "rust-toolchain.toml"
  - "AGENTS.md"
---

# TASK-101 — List saved PostgreSQL connections from a Turso store in a ratatui shell

Execution protocol, progress file grammar, and final report format are in `/task/AGENTS.md`. This file is read-only. It defines WHAT must become true; it does not change during execution.

## Goal

Running `pgtui` opens a TUI whose first screen lists the connections persisted in the local Turso SQLite file and exits cleanly with `q`.

## Context

Current behavior:

- `crates/pgtui/src/main.rs` is a stub that prints `error: not implemented` and exits 2; `lib.rs` exports only `pub mod render`.
- The trusted tests under `crates/pgtui/tests/` import `pgtui::store`, `pgtui::app`, `pgtui::keys`, `pgtui::ui` and do not compile.

Desired behavior:

- `pgtui --db <path>` opens (or creates) the store, enters raw mode/alternate screen, renders the `ConnectionList` screen (empty-state line or sorted rows), moves the cursor with `j`/`k`, and exits 0 on `q` or `Ctrl+C`; an unwritable `--db` exits 2 with `error: ...` on stderr before raw mode.

Read before editing (non-normative hints, in order):

1. `AGENTS.md` — build/test/lint commands.
2. `/task/decisions.md` — verbatim D-* text (layout, state machine, store API, keys, exit codes, frame). Decided; do not reopen.
3. `crates/pgtui/tests/support/mod.rs` — `render_text`, `render_svg`, `key`, `ctrl`, `temp_store` helpers the trusted tests use; shows the exact `App`/`Msg`/`Effect` surface they compile against.
4. `crates/pgtui/tests/store_test.rs`, `app_connection_list_test.rs`, `screen_connection_list_test.rs`, `cli_test.rs` — the oracles.
5. `crates/pgtui/src/render.rs` — planner-shipped Buffer→text/SVG; protected, reuse, do not copy.

Code flow: `main.rs` parses CLI (clap), resolves the store path (D-020), opens `ConnectionStore` (Turso, D-022), then runs the loop from D-012: draw via `ui::draw(frame, &app)`, read a key, `keys::action_for(screen, key)` maps it, `App::update(Msg) -> Vec<Effect>` is pure (D-011), `runtime::execute(&mut Runtime, Effect) -> Option<Msg>` does IO and feeds the reply back. Only `LoadConnections` and `Quit` are executed in this task; other effects are unreachable because `n` and `Enter` are no-ops here.

Baseline (run from repo root, before any edit):

```sh
cargo test -p pgtui --test store_test
```

Expected before this change: `error[E0432]: unresolved import` (`pgtui::store` does not exist).

## Preconditions

Each precondition has a command. If a command fails, stop and report `BLOCKED` (see AGENTS.md). Do not work around it.

- **P-001:** toolchain available — `cargo --version`
- **P-002:** test support present — `test -f crates/pgtui/tests/support/mod.rs`
- **P-003:** baseline tag resolvable — `git rev-parse --verify baseline`

## Scope

In scope:

- `store/mod.rs`: open, idempotent schema, `list`, `insert`, `DuplicateName`, `display_dsn`.
- `app.rs` core: `Screen`, `Msg`, `Effect`, `Status`, list navigation, quit; `keys.rs` for ConnectionList + global.
- `ui/mod.rs`, `ui/connection_list.rs`, `ui/status.rs`; `runtime.rs` for `LoadConnections`/`Quit`.
- `main.rs`: CLI, store path precedence, terminal lifecycle, exit codes 0/2.
- Executor unit tests (`#[cfg(test)]` in `src/`) for `keys::action_for` and store path resolution.

Out of scope:

- Create form (`n` is a no-op), connecting (`Enter` is a no-op), any PostgreSQL code, keyring, config file, themes.
- Dependency changes, formatting sweeps, unrelated warnings.

## Requirements

- **R-001 (MUST):** Store per D-020..D-022: schema executed on every open, `list` ordered by name ASC, unique-name violation → `StoreError::DuplicateName`, `created_at` RFC 3339 UTC set by the store.
- **R-002 (MUST):** App core per D-010..D-013; ConnectionList keys per D-030/D-031 (clamp, no wrap; `q`/`Ctrl+C` → `Effect::Quit`).
- **R-003 (MUST):** Render per D-060 ConnectionList including the empty state and the status line help text.
- **R-004 (MUST):** CLI and exit codes per D-040; unwritable `--db` → exit 2 and `error:` on stderr; `--version` → exit 0.
- **R-005 (MUST NOT):** Touch the store before `--help`/`--version` are handled.
- **R-006 (MUST):** Implement the final design directly. No compatibility layer, feature flag, or fallback.
- **R-007 (MUST NOT):** Add/upgrade dependencies or edit `Cargo.toml`, `Cargo.lock`, `justfile`, `rust-toolchain.toml`, `render.rs`, or anything under `tests/`. Executor tests live in `src/` only (D-003).

## Acceptance criteria

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given a fresh path, when the store opens, inserts two connections and reopens, then `list()` returns both sorted by name with RFC 3339 `created_at`. | `cargo test -p pgtui --test store_test` | exit 0, `5 passed` |
| AC-002 | Given a saved name, when inserted again, then `Err(StoreError::DuplicateName)`. | `cargo test -p pgtui --test store_test duplicate_name_rejected` | exit 0, `1 passed` |
| AC-003 | Given no connections, when rendered at 100×30, then text and SVG match `screen__connection_list_empty`. | `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_connection_list_test screen__connection_list_empty` | exit 0, `2 passed` |
| AC-004 | Given two connections with the cursor on the second, when rendered, then matches `screen__connection_list_two`. | `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_connection_list_test screen__connection_list_two` | exit 0, `2 passed` |
| AC-005 | Given the list screen, when `j`/`k`/`q`/`Ctrl+C` are pressed, then the cursor clamps and `Effect::Quit` is emitted. | `cargo test -p pgtui --test app_connection_list_test` | exit 0, `5 passed` |
| AC-006 | Given an unwritable `--db`, when `pgtui` starts, then exit 2 with `error:` on stderr; `--version` and `--help` exit 0. | `cargo test -p pgtui --test cli_test` | exit 0, `3 passed` |

## Fixed decisions

Already decided. Implement; do not reopen. Verbatim text in `/task/decisions.md`. Anything not listed there that changes public behavior, architecture, data, or security posture is `NEEDS_REPLAN`.

- **D-001..D-005:** workspace layout, module file map, test placement, lint gate, justfile.
- **D-010..D-013:** `Screen`/`App` fields, `Msg`/`Effect` enums, pure `update`, runtime loop, status line.
- **D-020..D-022:** Turso store path precedence, schema SQL, exact `ConnectionStore` API.
- **D-030, D-031:** global and ConnectionList keys.
- **D-040, D-041:** exit codes, error surfacing.
- **D-060, D-061:** 100×30 frame, ConnectionList layout strings, `ui::draw` entry point.
- **D-070, D-071:** protected test support and snapshot policy (`INSTA_UPDATE=no`, no `.snap.new`).

## Checklist

Static plan. IDs `N`…`N.N.N.N`, max depth 4, four spaces per level. State lives in `progress.md`.

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-003` pass — evidence: each command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test store_test` fails to compile with unresolved `pgtui::store` imports.
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Turso connection store (`R-001`, `D-020..D-022`) — evidence: `cargo test -p pgtui --test store_test` exits 0.
        - [ ] **2.1.1** `ConnectionStore::open` creates the D-021 schema idempotently — evidence: `cargo test -p pgtui --test store_test open_creates_schema` → `1 passed`.
        - [ ] **2.1.2** `insert` returns the saved row and `list` orders by name — evidence: `cargo test -p pgtui --test store_test insert_then_list_sorted_by_name` → `1 passed`.
        - [ ] **2.1.3** Unique-name violation maps to `StoreError::DuplicateName` — evidence: `cargo test -p pgtui --test store_test duplicate_name_rejected` → `1 passed`.
    - [ ] **2.2** App core and key mapping (`R-002`, `D-010`, `D-011`, `D-030`, `D-031`) — evidence: `cargo test -p pgtui --test app_connection_list_test` exits 0.
        - [ ] **2.2.1** `Msg::Connections` populates the list and clamps the cursor — evidence: `cargo test -p pgtui --test app_connection_list_test connections_msg_populates_list` → `1 passed`.
        - [ ] **2.2.2** `j`/`k` clamp at both ends; `q` and `Ctrl+C` emit `Effect::Quit` — evidence: `cargo test -p pgtui --test app_connection_list_test -- clamp quit` → `4 passed`.
    - [ ] **2.3** ConnectionList and status line render per `D-060` (`R-003`) — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_connection_list_test` → `4 passed`.
    - [ ] **2.4** CLI, store path precedence and exit codes (`R-004`, `R-005`, `D-020`, `D-040`) — evidence: `cargo test -p pgtui --test cli_test` → `3 passed`.
    - [ ] **2.5** Unit tests for `keys::action_for` and store path resolution exist in `src/` — evidence: `cargo test -p pgtui --lib -- --list | grep -c 'keys::tests\|store::tests'` prints ≥ 4.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001` and `AC-002` — evidence: `cargo test -p pgtui --test store_test` → `5 passed`.
    - [ ] **3.2** `AC-003` and `AC-004` — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_connection_list_test` → `4 passed`.
    - [ ] **3.3** `AC-005` — evidence: `cargo test -p pgtui --test app_connection_list_test` → `5 passed`.
    - [ ] **3.4** `AC-006` — evidence: `cargo test -p pgtui --test cli_test` → `3 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only `crates/pgtui/src/*` changed, nothing temporary or unrelated — evidence: `git status --porcelain` shows only those paths.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full verifier output shown in the transcript.
<!-- checklist:end -->
