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

Read before editing (orientation only, non-normative, in order):

1. `crates/pgtui/tests/skeleton_test.rs` — the oracle: stub behaviour plus the `pgtui::render` surface the scaffold must expose.
2. `crates/pgtui/tests/support/mod.rs` — the 100x30 buffer helpers it uses. Read-only; never edited.

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
- **R-003 (MUST):** `rust-toolchain.toml` pins `1.98.0`; `rustfmt.toml` sets `edition = "2024"`; `.gitignore` lists `target/`; `.cargo/config.toml` sets `TESTCONTAINERS_COMMAND = "remove"`.
- **R-004 (MUST):** `crates/pgtui/src/lib.rs` is exactly `pub mod render;`.
- **R-005 (MUST):** `pgtui` prints `error: not implemented` on stderr, nothing on stdout, exits 2; `gallery` prints usage on stderr, exits 2 (D-040).
- **R-006 (MUST):** Final design directly: no compatibility layer, dual path, deprecated alias, feature flag, or legacy fallback.
- **R-007 (MUST NOT):** Add, remove, or bump a dependency; edit `render.rs`, `fonts/`, or anything under `crates/pgtui/tests/`; create `justfile`, `README.md`, `AGENTS.md`.

## Acceptance criteria

Canonical typed acceptance blocks use taskfmt's Markdown profile. The blocks are task metadata, not
Cucumber feature files and have no runtime step definitions.

### AC-001 — Workspace builds
Type: scenario
Class: delta
Covers: R-001
Evidence: `cargo build --workspace --all-targets`
Expected: exit 0

```gherkin
Given the repository starts with the trusted scaffold
When the workspace is built with every target
Then the workspace compiles successfully
```

### AC-002 — Dependency pins are exact
Type: invariant
Class: policy
Covers: R-001
Evidence: `grep -qE '^ratatui = "=0\.30\.2"' Cargo.toml && grep -qE '^crossterm = "=0\.29\.0"' Cargo.toml && grep -qE '^tokio-postgres = "=0\.7\.18"' Cargo.toml && grep -qE '^turso = \{ version = "=0\.7\.2"' Cargo.toml && grep -qE '^tokio = \{ version = "=1\.53\.1"' Cargo.toml && grep -qE '^clap = \{ version = "=4\.6\.6"' Cargo.toml && grep -qE '^thiserror = "=2\.0\.20"' Cargo.toml && grep -qE '^directories = "=6\.0\.0"' Cargo.toml && grep -qE '^resvg = "=0\.48\.1"' Cargo.toml && grep -qE '^tempfile = "=3\.27\.0"' Cargo.toml && grep -qE '^testcontainers = "=0\.27\.3"' Cargo.toml && grep -qE '^testcontainers-modules = \{ version = "=0\.15\.0"' Cargo.toml && grep -qE '^nix = \{ version = "=0\.30\.1"' Cargo.toml`
Expected: exit 0

```gherkin
Given the workspace manifest
When its dependency declarations are inspected
Then every D-001 dependency uses its exact pinned version
```

### AC-003 — Toolchain metadata is configured
Type: invariant
Class: policy
Covers: R-003
Evidence: `grep -q '1.98.0' rust-toolchain.toml && grep -q 'edition = "2024"' rustfmt.toml && grep -q 'TESTCONTAINERS_COMMAND' .cargo/config.toml && grep -q 'target/' .gitignore`
Expected: exit 0

```gherkin
The channel, rustfmt edition, container environment, and ignore list match D-001.
```

### AC-004 — The pgtui stub has its exit contract
Type: scenario
Class: delta
Covers: R-005
Evidence: `cargo test -p pgtui --test skeleton_test -- stub`
Expected: exit 0, `2 passed`

```gherkin
Given the pgtui binary before later application tasks land
When its stub test runs
Then pgtui explains that it is not implemented on stderr and returns exit code 2
```

### AC-005 — The gallery stub has its exit contract
Type: scenario
Class: delta
Covers: R-005
Evidence: `cargo test -p pgtui --test skeleton_test -- stub`
Expected: exit 0, `2 passed`

```gherkin
Given the gallery binary before the gallery task lands
When its stub test runs
Then gallery prints usage on stderr and returns exit code 2
```

### AC-006 — The protected render pipeline works
Type: scenario
Class: invariant
Covers: R-004
Evidence: `cargo test -p pgtui --test skeleton_test -- trims pipeline`
Expected: exit 0, `2 passed`

```gherkin
Given the planner-shipped render module
When its text, SVG, and PNG pipeline tests run
Then all three representations behave as specified
```

### AC-007 — Completion gate passes
Type: gate
Evidence: `taskfmt verify`
Expected: exit 0, last line `DONE`

## Fixed decisions

Full text: `/task/decisions.md` (binding, read-only).
Implement; do not reopen. Anything not decided there that changes public behaviour, architecture, data or security posture is `NEEDS_REPLAN`, not executor discretion.

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
    - [ ] **2.1** Workspace builds with all targets (`R-001`, `D-001`) — evidence: `cargo build --workspace --all-targets --locked` exits 0.
        - [ ] **2.1.1** Workspace manifest with exact pins (`R-001`, `AC-002`) — evidence: the `AC-002` pin check exits 0.
        - [ ] **2.1.2** Member manifest declares lib plus two bins with `workspace = true` dependencies (`R-002`) — evidence: `grep -q 'name = "gallery"' crates/pgtui/Cargo.toml && grep -q '\[\[bin\]\]' crates/pgtui/Cargo.toml` exits 0.
        - [ ] **2.1.3** Toolchain, rustfmt, ignore list and container env per `R-003` (`AC-003`) — evidence: the `AC-003` toolchain check exits 0.
        - [ ] **2.1.4** Workspace build succeeds (`AC-001`) — evidence: `cargo build --workspace --all-targets` exits 0.
    - [ ] **2.2** `lib.rs` exports only the protected render module (`R-004`, `D-002`) — evidence: `test "$(tr -d '[:space:]' < crates/pgtui/src/lib.rs)" = 'pubmodrender;'` exits 0.
    - [ ] **2.3** The pgtui stub is exit 2 (`R-005`, `AC-004`, `D-040`) — evidence: `cargo test -p pgtui --test skeleton_test -- stub` prints `2 passed` for pgtui.
- [ ] **3** Trusted material is untouched and proven.
    - [ ] **3.1** The gallery stub is exit 2 (`R-005`, `AC-005`, `D-040`) — evidence: `cargo test -p pgtui --test skeleton_test -- stub` prints `2 passed` for gallery.
    - [ ] **3.2** Render pipeline behaves as stated (`AC-006`) — evidence: `cargo test -p pgtui --test skeleton_test -- trims pipeline` prints `2 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated (`R-006`, `R-007`) — evidence: `git status --porcelain` and `git diff --no-renames --stat $TASKFMT_BASE` show only in-scope files.
    - [ ] **4.2** `taskfmt verify` exits 0 with last line `DONE` (`AC-007`) — evidence: final full run (with progress check), full output shown in the transcript.
<!-- checklist:end -->
