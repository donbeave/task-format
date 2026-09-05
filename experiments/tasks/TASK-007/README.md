---
schema: task/v4
id: TASK-007
title: "Disconnect, exit codes and the screen gallery"
kind: feature
verify: "taskfmt verify"
expected_paths:
  - "Cargo.toml"
  - "Cargo.lock"
  - "crates/pgtui/Cargo.toml"
  - "crates/pgtui/src/app.rs"
  - "crates/pgtui/src/keys.rs"
  - "crates/pgtui/src/runtime.rs"
  - "crates/pgtui/src/main.rs"
  - "crates/pgtui/src/db/mod.rs"
  - "crates/pgtui/src/db/postgres.rs"
  - "crates/pgtui/src/bin/gallery.rs"
  - "README.md"
  - "docs/screens/*"
---

# TASK-007 — Disconnect, exit codes and the screen gallery

Execution protocol, progress grammar, report format: `/task/AGENTS.md`. This file is read-only and states WHAT must become true.

## Goal

`d` disconnects cleanly, `q`/`Ctrl+C` exit 0 from a live session in a real pty, and `gallery` writes the ten named screens as deterministic SVG+PNG pairs plus the committed `docs/screens` run and repository `README.md`.

## Context

Current behavior:

- TASK-006 completed the app surface; `d` is inert, disconnect never closes the socket, `gallery` is the TASK-001 exit-2 stub, and there is no `README.md` or `docs/screens/`.
- Trusted tests `app_disconnect_test.rs`, `pg_disconnect_test.rs`, `gallery_test.rs` and `cli_exit_test.rs` are on the run base commit and none of them passes there: two of the four do not build at base (`cli_exit_test` for want of the `nix` dev-dependency line; `pg_disconnect_test` for want of `PgSession::disconnect`), the other two fail while the feature is absent. Adding the `nix` D-001 dev pin to `crates/pgtui/Cargo.toml` is part of this task (`R-007`).

Desired behavior:

- Disconnect (D-033/D-024), exit codes (D-040/D-012) and gallery (D-080) behave exactly as R-001..R-004 state and AC-001..AC-004 prove.

Read before editing (orientation only, non-normative, in order):

1. `crates/pgtui/tests/app_disconnect_test.rs` and `crates/pgtui/tests/pg_disconnect_test.rs` — the disconnect oracles (state reset, backend actually closed).
2. `crates/pgtui/tests/cli_exit_test.rs` and `crates/pgtui/tests/gallery_test.rs` — pty exit codes and the ten-screen contract.

Code flow: `keys.rs` routes `d` in the browser to `Effect::Disconnect`; `runtime::execute` removes the `PgSession`, drops the `Client`, awaits the spawned connection's `JoinHandle`, and replies `Msg::Disconnected`. `main.rs` keeps the D-012 loop, with `Ctrl+C` emitting `Disconnect` before `Quit` (D-030). `src/bin/gallery.rs` builds the ten D-080 states locally, renders through `pgtui::render`, and writes `<name>.svg`/`<name>.png`.

Baseline, from the repo root:

```sh
cargo test -p pgtui --test gallery_test
```

Expected before this change: failure — `gallery` is the exit-2 stub.

## Preconditions

If a command fails, stop and report `BLOCKED`. Never work around a precondition. Docker is required.

- **P-001:** toolchain available — `cargo --version`
- **P-002:** TASK-006 suite green — `cargo test -p pgtui --test app_custom_sql_test`
- **P-003:** docker reachable — `docker info >/dev/null`
- **P-004:** trusted gallery tests present — `test -f crates/pgtui/tests/gallery_test.rs`
- **P-005:** working tree clean — `test -z "$(git status --porcelain)"`

## Scope

In scope:

- Disconnect in `keys.rs`/`runtime.rs`/`app.rs`, socket teardown in `db/postgres.rs`, exit codes in `main.rs`, `src/bin/gallery.rs`, `docs/screens/`, `README.md`.

Out of scope:

- Store, connect, preview and custom-SQL behaviour; frozen by earlier tasks.
- `render.rs`, `fonts/`, anything under `crates/pgtui/tests/`.
- CI, packaging, theming, keyring.

## Requirements

- **R-001 (MUST):** Disconnect per D-033/D-024: `d` from the browser emits `Effect::Disconnect`; `Msg::Disconnected` sets `session = None`, `grid = None`, `sql_grid = None`, returns to `Screen::ConnectionList`, keeps `connections` and `list_cursor`, and clears `sql_input`.
- **R-002 (MUST):** Teardown closes the backend: the `Client` is dropped and the spawned connection task is awaited before `Msg::Disconnected` is returned, so `pg_stat_activity` shows no `application_name = 'pgtui'` backend other than the observing connection's own afterwards.
- **R-003 (MUST):** `Ctrl+C` emits `Disconnect` before `Quit` when connected, from every screen (D-030); `q` and `Ctrl+C` exit 0 with the terminal restored (D-040, D-012).
- **R-004 (MUST):** Gallery per D-080: `--out <dir>` defaulting to `docs/screens`, the ten fixed names, SVG plus PNG per name, deterministic bytes, exit 0 on success, 2 on bad arguments or unwritable output, 1 on render failure, no rendering code of its own, nothing `#[path]`-included from `tests/`.
- **R-005 (MUST):** `docs/screens/` holds one committed run with the default `--out`, and `README.md` gets a `## Screens` section listing the ten names with their `docs/screens/<name>.png` paths (D-005: this is the only task creating `README.md`).
- **R-006 (MUST):** Final design directly; no compatibility layer, dual path, deprecated alias, feature flag, or legacy fallback.
- **R-007 (MUST NOT):** Remove or bump a D-001 pin, or depend on anything outside the D-001 pin list (adding a listed pin is required where trusted tests need it); use `sort_unstable`; touch `render.rs`, `fonts/`, or anything under `crates/pgtui/tests/`; change any frozen behaviour; create `justfile`.

## Acceptance criteria

Canonical typed acceptance blocks use taskfmt's Markdown profile. They are task metadata, not
Cucumber feature files and have no runtime step definitions.

### AC-001 — Disconnect resets the app
```gherkin
Given a connected App with saved connections and a selected list row
When d or Ctrl+C is applied to the browser
Then the disconnect effect and decided disconnected state are produced
And the session, grids, and SQL input reset while connections and list cursor remain
```

**Verification**

- **Type:** scenario
- **Covers:** `R-001`
- **Expected:** exit 0, `5 passed`

```sh
cargo test -p pgtui --test app_disconnect_test
```

### AC-002 — Disconnect closes the backend
```gherkin
Given a live Postgres session in a container
When disconnect runs
Then the pgtui backend is absent from pg_stat_activity before the reply returns
```

**Verification**

- **Type:** scenario
- **Covers:** `R-002`
- **Expected:** exit 0, `1 passed`

```sh
cargo test -p pgtui --test pg_disconnect_test
```

### AC-003 — Interactive exits restore the terminal
```gherkin
Given the binary in a 100x30 pty
When q and Ctrl+C each end a session
Then both processes exit 0 and leave the alternate screen
```

**Verification**

- **Type:** scenario
- **Covers:** `R-003`
- **Expected:** exit 0, `2 passed`

```sh
cargo test -p pgtui --test cli_exit_test
```

### AC-004 — Gallery output is deterministic
```gherkin
Given the gallery binary and its default output directory
When it runs twice
Then it writes the ten named SVG and PNG pairs with byte-identical output
```

**Verification**

- **Type:** scenario
- **Covers:** `R-004`
- **Expected:** exit 0, `4 passed`

```sh
cargo test -p pgtui --test gallery_test
```

### AC-005 — The complete trusted series remains green
```gherkin
Given the complete trusted test series for the task
When all 23 targets are run
Then every target reports test result ok and no target reports FAILED
```

**Verification**

- **Type:** invariant
- **Covers:** `R-001, R-002, R-003, R-004, R-005`
- **Expected:** exit 0, 23 `test result: ok.` lines (one per target), no `FAILED`

```sh
cargo test -p pgtui \
  --test store_test \
  --test app_connection_list_test \
  --test screen_connection_list_test \
  --test cli_test \
  --test app_create_form_test \
  --test runtime_create_test \
  --test screen_create_form_test \
  --test pg_connect_test \
  --test pg_runtime_connect_test \
  --test app_browser_test \
  --test screen_browser_test \
  --test grid_sort_test \
  --test pg_preview_test \
  --test app_preview_test \
  --test screen_preview_test \
  --test app_custom_sql_test \
  --test pg_custom_sql_test \
  --test screen_custom_sql_test \
  --test app_disconnect_test \
  --test pg_disconnect_test \
  --test cli_exit_test \
  --test gallery_test \
  --test skeleton_test -- --skip stub_exits_2
```

### AC-006 — Completion gate passes
**Verification**

- **Type:** gate
- **Expected:** exit 0, last line `DONE`

```sh
taskfmt verify
```

## Fixed decisions

Full text: `/task/decisions.md` (binding, read-only).
Implement; do not reopen. Anything not decided there that changes public behaviour, architecture, data or security posture is `NEEDS_REPLAN`, not executor discretion.

- **D-024, D-033:** disconnect semantics, socket teardown, `d` key.
- **D-012, D-030, D-040:** loop, global `Ctrl+C`, exit codes.
- **D-080:** gallery contract, the ten names, determinism, exit codes. **D-005:** `README.md` comes from this task only.
- **D-070, D-071:** trusted helpers and behavioural verification.

## Checklist

Static plan. Hierarchical IDs, four spaces per level, max depth 4. Every leaf names what becomes true and the evidence permitting the check.

<!-- checklist:start -->
- [ ] **1** Baseline and environment are reproduced.
    - [ ] **1.1** Preconditions `P-001..P-005` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test gallery_test` fails because `gallery` writes nothing.
- [ ] **2** Disconnect is implemented.
    - [ ] **2.1** `d` emits `Effect::Disconnect` and `Msg::Disconnected` resets state per D-033 (`R-001`, `AC-001`) — evidence: `cargo test -p pgtui --test app_disconnect_test` prints `5 passed`.
    - [ ] **2.2** Teardown closes the backend before replying (`R-002`, `AC-002`) — evidence: `cargo test -p pgtui --test pg_disconnect_test` prints `1 passed`.
- [ ] **3** Exit behaviour is proven.
    - [ ] **3.1** `q` exits 0 from a live session in a 100x30 pty with the terminal restored (`R-003`, `AC-003`) — evidence: `cargo test -p pgtui --test cli_exit_test -- q` prints `1 passed`.
    - [ ] **3.2** `Ctrl+C` exits 0 from the same pty (`R-003`) — evidence: `cargo test -p pgtui --test cli_exit_test -- ctrl_c` prints `1 passed`.
- [ ] **4** Gallery and repository material are produced.
    - [ ] **4.1** `src/bin/gallery.rs` meets D-080 without its own rendering code (`R-004`, `AC-004`) — evidence: `cargo test -p pgtui --test gallery_test` prints `4 passed`.
    - [ ] **4.2** `docs/screens/` holds the committed default-`--out` run and `README.md` carries its `## Screens` section (`R-005`) — evidence: `test "$(ls docs/screens/*.png | wc -l | tr -d ' ')" = 10 && test "$(ls docs/screens/*.svg | wc -l | tr -d ' ')" = 10 && grep -q '## Screens' README.md` exits 0. The ten NAMES and their paths are fixed by the gallery contract; this leaf owns the committed run and the section, and nothing that another oracle decides.
    - [ ] **4.3** Lint is clean (`D-004`) — evidence: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] **5** Gate passes.
    - [ ] **5.1** Full trusted series holds — evidence: the `AC-005` command exits 0 and prints 23 `test result: ok.` lines, one per `--test` target, and no `FAILED`.
    - [ ] **5.2** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated (`R-006`, `R-007`) — evidence: `git status --porcelain` and `git diff --no-renames --stat $TASKFMT_BASE` show only in-scope files.
    - [ ] **5.3** `taskfmt verify` exits 0 with last line `DONE` (`AC-006`) — evidence: final full run (with progress check), full output shown in the transcript.
<!-- checklist:end -->
