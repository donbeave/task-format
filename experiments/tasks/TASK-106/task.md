---
schema: task/v3
id: TASK-106
title: "Disconnect returns to the list, exit restores the terminal, gallery renders every screen"
kind: feature
verify: "/task/verify.sh"
expected_paths:
  - "crates/pgtui/src/app.rs"
  - "crates/pgtui/src/keys.rs"
  - "crates/pgtui/src/runtime.rs"
  - "crates/pgtui/src/main.rs"
  - "crates/pgtui/src/db/postgres.rs"
  - "crates/pgtui/src/bin/gallery.rs"
  - "docs/screens/*"
  - "README.md"
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

# TASK-106 — Disconnect returns to the list, exit restores the terminal, gallery renders every screen

Execution protocol, progress file grammar, and final report format are in `/task/AGENT.md`. Verbatim fixed-decision text is in `/task/decisions.md`. Both are read-only, as is this file.

## Goal

`d` closes the PostgreSQL session and returns to the connection list, `q`/`Ctrl+C` exit with code 0 from any state with the terminal restored, and `cargo run -p pgtui --bin gallery` writes one SVG and one PNG per screen into `docs/screens/`.

## Context

Current behavior:

- `d` in the Browser is a no-op; `Effect::Disconnect` and `Msg::Disconnected` exist (TASK-101 enums) but are never emitted, executed, or handled. `Ctrl+C` emits `[Quit]` only, so the PG connection task is dropped without a clean close.
- `PgSession::connect` does not set `application_name`, so a test cannot count the app's backends in `pg_stat_activity`.
- `main.rs` restores the terminal on normal exit only; a panic leaves raw mode on.
- `crates/pgtui/src/bin/gallery.rs` is a stub that prints usage and exits 2; `docs/screens/` does not exist. Trusted test `app_disconnect_test::d_disconnects` fails.

Desired behavior:

- `d` → `Effect::Disconnect`; the runtime drops the session and awaits the connection task; `Msg::Disconnected` resets `session`, `grid`, `sql_grid` to `None`, screen `ConnectionList`, `list_cursor` preserved.
- `Ctrl+C` while connected yields exactly `[Disconnect, Quit]` from every screen; both quit paths exit 0 after leaving the alternate screen and disabling raw mode; a panic hook restores the terminal before the default hook runs.
- `gallery --out <dir>` renders the ten D-071 screens from `fake_data`-equivalent state through `pgtui::render` into `<dir>/<name>.svg` and `<dir>/<name>.png`; SVG bytes equal the protected `__svg` snapshots and are identical across runs; `docs/screens/` holds the committed output and `README.md` lists it.

Read before editing (non-normative hints, in order):

1. `CLAUDE.md` — build, test, lint commands.
2. `/task/decisions.md` — D-002, D-012, D-024, D-030, D-033, D-040, D-041, D-070, D-071, D-080 verbatim; decided, do not reopen.
3. `crates/pgtui/src/render.rs` — protected; the only render implementation. The bin calls it; it must not copy it.
4. `crates/pgtui/src/runtime.rs`, `crates/pgtui/src/db/postgres.rs` — where the session and its connection task live.
5. `crates/pgtui/src/main.rs` — terminal setup/teardown and exit codes.
6. `crates/pgtui/tests/screen_*_test.rs` — how each of the ten snapshot states is built; the gallery must build the same states.
7. `crates/pgtui/tests/{app_disconnect,pg_disconnect,cli_exit,gallery}_test.rs` — the oracle.

Code flow: `keys::action_for` maps `d` (Browser) to a disconnect action and `Ctrl+C` (global) to quit; `App::update` turns quit into `[Disconnect, Quit]` when `session.is_some()`. `runtime::execute(Disconnect)` takes the `PgSession` out of the runtime, drops the client, awaits the spawned connection future, and replies `Msg::Disconnected`. `main.rs` installs a panic hook that restores the terminal, runs the loop, restores the terminal, and returns `0`. `bin/gallery.rs` constructs the ten `App` states (the same states `screen_*_test.rs` build), renders each with `pgtui::render`, and writes the files.

Baseline (run from repo root, before any edit):

```sh
cargo test -p pgtui --test app_disconnect_test
```

Expected before this change: `d_disconnects` fails — `d` is a no-op (`screen == Browser`, `session.is_some()`).

## Preconditions

Each precondition has a command. If a command fails, stop and report `BLOCKED` (see AGENT.md). Do not work around it.

- **P-001:** toolchain available — `cargo --version`
- **P-002:** protected test support present — `test -f crates/pgtui/tests/support/mod.rs`
- **P-003:** baseline tag resolvable — `git rev-parse --verify baseline`
- **P-004:** Docker reachable (testcontainers) — `docker info >/dev/null`

## Scope

In scope:

- `Effect::Disconnect` execution and `Msg::Disconnected` handling; `Ctrl+C` ordering.
- `application_name=pgtui` in the connection config.
- Panic hook and terminal restore in `main.rs`; exit code 0 on both quit paths.
- `src/bin/gallery.rs`, `docs/screens/*` (20 files), README screen list.

Out of scope:

- New screens, reconnect, theming, CI, any change to `render.rs`, `grid.rs`, `store/`, keyring, dependency changes.

## Requirements

- **R-001 (MUST):** `d` in Browser → `Effect::Disconnect`; runtime drops `PgSession` and awaits its connection task; reply `Msg::Disconnected`; app resets per D-033. Connection config includes `application_name=pgtui` (D-024).
- **R-002 (MUST):** `Ctrl+C` while connected emits exactly `[Effect::Disconnect, Effect::Quit]` in that order, from every screen (D-030); disconnected it emits `[Effect::Quit]`.
- **R-003 (MUST):** Exit code 0 on `q` and `Ctrl+C`; raw mode off and alternate screen left on normal exit and on panic via a panic hook (D-040). `std::process::exit` only in `main.rs`.
- **R-004 (MUST):** `gallery --out <dir>` (default `docs/screens`) writes `<name>.svg` and `<name>.png` for the ten D-071 names; SVG bytes are byte-identical across runs and equal the protected `__svg` snapshot bodies.
- **R-005 (MUST NOT):** Duplicate render code. The bin uses `pgtui::render` only.
- **R-006 (MUST):** `docs/screens/` contains the 20 committed files; `README.md` gains a section listing all ten screens with their `docs/screens/` paths.
- **R-007 (MUST):** Implement the final design directly. No compatibility layer, dual path, feature flag, or fallback.
- **R-008 (MUST NOT):** Add or upgrade dependencies; edit `Cargo.toml`, `Cargo.lock`, `justfile`, `rust-toolchain.toml`, protected `src/` files, or anything under `crates/pgtui/tests/`; use `#[ignore]`, `todo!`, `unimplemented!`, `dbg!`, or `#[allow(...)]`.

## Acceptance criteria

Each row is observable behavior with the exact evidence command. `verify.sh` runs these; the harness re-runs them.

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given Browser with a grid, when `d` then `Msg::Disconnected`, then `ConnectionList`, `session`/`grid`/`sql_grid` are `None`, `list_cursor` preserved. | `cargo test -p pgtui --test app_disconnect_test -- --skip ctrl_c` | exit 0, `4 passed` |
| AC-002 | Given the container, when the runtime executes `Connect` then `Disconnect`, then `SELECT count(*) FROM pg_stat_activity WHERE application_name = 'pgtui'` on an admin connection returns 0. | `cargo test -p pgtui --test pg_disconnect_test` | exit 0, `1 passed` |
| AC-003 | Given `Ctrl+C` in CustomSql while connected, when handled, then effects are exactly `[Disconnect, Quit]`. | `cargo test -p pgtui --test app_disconnect_test ctrl_c_disconnects_then_quits` | exit 0, `1 passed` |
| AC-004 | Given the binary under a pty with an empty store, when `q` (or `Ctrl+C`) is sent, then exit 0 and the output contains the leave-alternate-screen sequence. | `cargo test -p pgtui --test cli_exit_test` | exit 0, `2 passed` |
| AC-005 | Given `cargo run -p pgtui --bin gallery -- --out target/gallery` run twice, then 20 files exist, every SVG equals the protected `__svg` snapshot body, and both runs are byte-identical. | `cargo test -p pgtui --test gallery_test` | exit 0, `3 passed` |
| AC-006 | Given the whole suite, when every `screen_*` test file runs, then every snapshot from TASK-101..105 still passes. | `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test 'screen_*'` | exit 0, `22 passed` in total |

## Fixed decisions

Already decided; full text in `/task/decisions.md`. Implement; do not reopen. Anything not listed that changes public behavior, architecture, data, or security posture is `NEEDS_REPLAN`, not executor discretion.

- **D-002:** module layout; `bin/gallery.rs` location; `render.rs` protected.
- **D-012:** runtime loop and effect execution order.
- **D-024:** connection config (key/value, `NoTls`, 5 s timeout) plus `application_name=pgtui`.
- **D-030, D-033:** `Ctrl+C` global ordering; `d` and `Disconnected` reset.
- **D-040, D-041:** exit codes 0/1/2; terminal restored before any exit; errors never swallowed.
- **D-070, D-071:** protected test support; the ten snapshot names and `__svg` bodies.
- **D-080:** `gallery` CLI contract (args, file names, exit codes, state construction).

## Checklist

Static plan. IDs `N`…`N.N.N.N`, max depth 4, four spaces per level. Every leaf names what becomes true and its evidence. State lives in `progress.md`, never here.

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-004` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test app_disconnect_test` → `d_disconnects` fails.
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Disconnect flow (`R-001`, `R-002`, `D-030`, `D-033`) — evidence: `cargo test -p pgtui --test app_disconnect_test` → `5 passed`.
        - [ ] **2.1.1** `d` emits `Effect::Disconnect`; `Disconnected` resets session state — evidence: `cargo test -p pgtui --test app_disconnect_test d_disconnects disconnected_resets` → `2 passed`.
        - [ ] **2.1.2** `Ctrl+C` while connected yields `[Disconnect, Quit]` — evidence: `cargo test -p pgtui --test app_disconnect_test ctrl_c_disconnects_then_quits` → `1 passed`.
    - [ ] **2.2** Runtime closes the PG session and sets `application_name=pgtui` (`R-001`, `D-024`) — evidence: `cargo test -p pgtui --test pg_disconnect_test` → `1 passed`.
    - [ ] **2.3** Terminal lifecycle and exit code 0 on both quit paths; panic hook restores the terminal (`R-003`, `D-040`) — evidence: `cargo test -p pgtui --test cli_exit_test` → `2 passed`.
    - [ ] **2.4** `gallery` binary renders all ten screens via `pgtui::render` (`R-004`, `R-005`, `D-080`) — evidence: `cargo test -p pgtui --test gallery_test` → `3 passed`.
        - [ ] **2.4.1** `--out <dir>` creates 10 SVG + 10 PNG files with the D-071 names — evidence: `cargo test -p pgtui --test gallery_test writes_twenty_files` → `1 passed`.
        - [ ] **2.4.2** SVGs match protected `__svg` snapshots and are stable across runs — evidence: `cargo test -p pgtui --test gallery_test svg_matches_snapshots svg_deterministic` → `2 passed`.
    - [ ] **2.5** `docs/screens/` committed output and README screen list (`R-006`) — evidence: `ls docs/screens | wc -l` → `20`; `grep -c 'docs/screens/' README.md` → `10` or more.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001`, `AC-003` — evidence: `cargo test -p pgtui --test app_disconnect_test` → `5 passed`.
    - [ ] **3.2** `AC-002`, `AC-004` — evidence: `cargo test -p pgtui --test pg_disconnect_test --test cli_exit_test` → `3 passed` in total.
    - [ ] **3.3** `AC-005`, `AC-006` — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test gallery_test --test 'screen_*'` → `25 passed` in total.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated — evidence: `git status --porcelain` and `git diff --stat baseline` show only in-scope files.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full verifier output shown in the transcript.
<!-- checklist:end -->
