---
schema: task/v4
id: TASK-001
title: "Bootstrap the pgtui workspace from an empty main"
kind: feature
verify: "taskfmt verify"
expected_paths:
  - "Cargo.toml"
  - "Cargo.lock"
  - "rust-toolchain.toml"
  - "rustfmt.toml"
  - ".gitignore"
  - ".cargo/config.toml"
  - "crates/pgtui/Cargo.toml"
  - "crates/pgtui/src/lib.rs"
  - "crates/pgtui/src/main.rs"
  - "crates/pgtui/src/bin/gallery.rs"
---

# TASK-001 — Bootstrap the pgtui workspace from an empty main

Execution protocol, progress grammar, report format: `/task/AGENTS.md`. This file is read-only and states WHAT must become true, never how the work is narrated.

## Goal

A repository whose `main` started empty builds with `cargo build --workspace --all-targets`, every dependency pinned, and both binaries are the specified exit-2 stubs.

## Context

Current behavior:

- `main` holds one allow-empty bootstrap commit and no files: no `Cargo.toml`, so no cargo command works at the root.
- Trusted verification material is already committed on the run base commit: `crates/pgtui/src/render.rs` with `crates/pgtui/src/fonts/DejaVuSansMono.ttf`, `crates/pgtui/tests/support/mod.rs`, `crates/pgtui/tests/skeleton_test.rs`.

Desired behavior:

- Single-member workspace builds `pgtui` (lib + bin) and `gallery`. `pgtui` prints `error: not implemented` on stderr and exits 2; `gallery` prints usage on stderr and exits 2. `crates/pgtui/src/lib.rs` is exactly `pub mod render;` so the planner-shipped render module compiles.

Read before editing (in order):

1. `/task/decisions.md` — verbatim D-* text: workspace and pins (D-001), module map (D-002), test placement (D-003). Decided; do not reopen.
2. `crates/pgtui/tests/skeleton_test.rs` — the oracle: stub behaviour plus the `pgtui::render` surface the scaffold must expose.
3. `crates/pgtui/tests/support/mod.rs` — the 100x30 buffer helpers it uses. Read-only; never edited.

Code flow: the workspace `Cargo.toml` owns `[workspace.dependencies]` pinned at `=` (D-001) and one member `crates/pgtui`; that manifest sets `workspace = true` for all dependencies and declares `[lib]`, `[[bin]] pgtui`, `[[bin]] gallery`. `src/lib.rs` re-exports the planner-shipped `render`; `main.rs` and `src/bin/gallery.rs` are exit-2 stubs (D-040) until later tasks replace them.

Baseline, run from the repo root before any edit:

```sh
cargo test -p pgtui --test skeleton_test
```

Expected before this change: `error: could not find Cargo.toml` — no workspace exists yet.

## Preconditions

If a command fails, stop and report `BLOCKED`. Never work around a precondition.

- **P-001:** toolchain available — `cargo --version`
- **P-002:** trusted render module present — `test -f crates/pgtui/src/render.rs`
- **P-003:** bundled font present — `test -f crates/pgtui/src/fonts/DejaVuSansMono.ttf`
- **P-004:** trusted tests present — `test -f crates/pgtui/tests/skeleton_test.rs`
- **P-005:** working tree clean — `test -z "$(git status --porcelain)"`

## Scope

In scope:

- Workspace `Cargo.toml` with exact pins, `crates/pgtui/Cargo.toml`, committed `Cargo.lock`.
- `rust-toolchain.toml`, `rustfmt.toml`, `.gitignore`, `.cargo/config.toml`.
- `crates/pgtui/src/lib.rs`, the `pgtui` stub, the `gallery` stub.

Out of scope:

- Any application module (`app.rs`, `store/`, `db/`, `grid.rs`, `runtime.rs`, `ui/`); later tasks add them.
- `render.rs`, `fonts/`, anything under `crates/pgtui/tests/`: planner-shipped, outside the whitelist.
- `README.md`, CI, theming, keyring.

## Requirements

- **R-001 (MUST):** Workspace per D-001: `resolver = "3"`, `edition = "2024"`, one member `crates/pgtui`, every dependency in `[workspace.dependencies]` at the exact pinned version and features.
- **R-002 (MUST):** `crates/pgtui/Cargo.toml` uses `workspace = true` for every dependency and declares `[lib]`, `[[bin]] pgtui`, `[[bin]] gallery`.
- **R-003 (MUST):** `rust-toolchain.toml` pins `1.98.0`; `rustfmt.toml` sets `edition = "2024"`; `.gitignore` lists `target/` and `*.snap.new`; `.cargo/config.toml` sets `TESTCONTAINERS_COMMAND = "remove"`.
- **R-004 (MUST):** `crates/pgtui/src/lib.rs` is exactly `pub mod render;`.
- **R-005 (MUST):** `pgtui` prints `error: not implemented` on stderr, nothing on stdout, exits 2; `gallery` prints usage on stderr, exits 2 (D-040).
- **R-006 (MUST):** Final design directly: no compatibility layer, dual path, deprecated alias, feature flag, or legacy fallback.
- **R-007 (MUST NOT):** Add, remove, or bump a dependency; edit `render.rs`, `fonts/`, or anything under `crates/pgtui/tests/`; create `justfile`, `README.md`, `AGENTS.md`.

## Acceptance criteria

Observable behaviour plus the exact evidence command. The gate runs these; the harness re-runs them.

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given the empty repository, when the workspace exists, then everything builds with all targets. | `cargo build --workspace --all-targets` | exit 0 |
| AC-002 | Given the workspace manifest, when inspected, then D-001 pins are present at `=`. | `grep -q 'ratatui = "=0.30.2"' Cargo.toml && grep -q 'tokio-postgres = "=0.7.18"' Cargo.toml && grep -q 'turso' Cargo.toml` | exit 0 |
| AC-003 | Given the toolchain files, when inspected, then channel, rustfmt edition, container env and ignore list match D-001. | `grep -q '1.98.0' rust-toolchain.toml && grep -q 'edition = "2024"' rustfmt.toml && grep -q 'TESTCONTAINERS_COMMAND' .cargo/config.toml && grep -q 'target/' .gitignore` | exit 0 |
| AC-004 | Given the two binaries, when run, then both explain themselves on stderr and exit 2. | `cargo test -p pgtui --test skeleton_test stub` | exit 0, `2 passed` |
| AC-005 | Given the planner-shipped render module, when its tests run, then text, SVG and PNG behave as stated. | `cargo test -p pgtui --test skeleton_test -- trims pipeline` | exit 0, `2 passed` |
| AC-006 | Given the finished scaffold, when the gate runs, then it reports `DONE`. | `taskfmt verify` | exit 0, last line `DONE` |

## Fixed decisions

Implement; do not reopen. Verbatim text in `/task/decisions.md`. Anything not decided there that changes public behaviour, architecture, data or security posture is `NEEDS_REPLAN`, not executor discretion.

- **D-001:** workspace layout, toolchain pin, exact dependency pins and features, committed `Cargo.lock`.
- **D-002:** module file map; `render.rs` and `fonts/` are planner-shipped, the rest arrive later.
- **D-003:** trusted tests live in `crates/pgtui/tests/` and are read-only; executor tests are `#[cfg(test)]` in `src/`.
- **D-004:** toolchain and lint gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`.
- **D-040:** exit code 2 is the stub contract for both binaries.
- **D-070, D-071:** verification material is planner-shipped and asserts behaviour, never golden bytes.

## Checklist

Static plan. Hierarchical IDs `N`, `N.N`, `N.N.N`, `N.N.N.N`, four spaces per level, max depth 4. Every leaf names what becomes true and the evidence permitting the check. State lives in `progress.md`, never here.

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-005` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test skeleton_test` fails with `could not find Cargo.toml`.
- [ ] **2** Required structure exists.
    - [ ] **2.1** Workspace manifest with exact pins (`R-001`, `AC-002`, `D-001`) — evidence: `grep -q 'ratatui = "=0.30.2"' Cargo.toml && grep -q 'tokio-postgres = "=0.7.18"' Cargo.toml` exits 0.
        - [ ] **2.1.1** Member manifest declares lib plus two bins with `workspace = true` dependencies (`R-002`) — evidence: `grep -q 'name = "gallery"' crates/pgtui/Cargo.toml && grep -q '\[\[bin\]\]' crates/pgtui/Cargo.toml` exits 0.
        - [ ] **2.1.2** Toolchain, rustfmt, ignore list and container env per `R-003` (`AC-003`) — evidence: `grep -q 'TESTCONTAINERS_COMMAND' .cargo/config.toml && grep -q 'edition = "2024"' rustfmt.toml` exits 0.
        - [ ] **2.1.3** `Cargo.lock` committed and in sync (`R-001`) — evidence: `test -f Cargo.lock && cargo metadata --format-version 1 >/dev/null` exits 0.
    - [ ] **2.2** `lib.rs` exports only the protected render module (`R-004`, `D-002`) — evidence: `test "$(tr -d '[:space:]' < crates/pgtui/src/lib.rs)" = 'pubmodrender;'` exits 0.
    - [ ] **2.3** Both binaries are exit-2 stubs (`R-005`, `AC-004`, `D-040`) — evidence: `cargo test -p pgtui --test skeleton_test stub` prints `2 passed`.
- [ ] **3** Trusted material is untouched and proven.
    - [ ] **3.1** `render.rs` and `fonts/` unmodified (`R-007`, `D-003`) — evidence: `git diff --stat -- crates/pgtui/src/render.rs crates/pgtui/src/fonts` prints nothing.
    - [ ] **3.2** Render pipeline behaves as stated (`AC-005`) — evidence: `cargo test -p pgtui --test skeleton_test -- trims pipeline` prints `2 passed`.
- [ ] **4** Acceptance criteria are proven.
    - [ ] **4.1** `AC-001` — evidence: `cargo build --workspace --all-targets` exits 0.
    - [ ] **4.2** `AC-004` and `AC-005` — evidence: `cargo test -p pgtui --test skeleton_test` prints `4 passed`.
- [ ] **5** Gate passes.
    - [ ] **5.1** Diff reviewed: only `expected_paths` changed — evidence: `git status --porcelain` lists only in-scope files.
    - [ ] **5.2** Gate green (`AC-006`) — evidence: `taskfmt verify` exits 0 with last line `DONE`.
<!-- checklist:end -->
