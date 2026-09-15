# Taskfmt workspace crate split — execution plan

**Pass to agent:**

```text
/goal Implement the taskfmt workspace crate split exactly as specified in docs/crate-split.md. Phase 1 may already be applied on disk (uncommitted); verify against this document, finish any gaps, then complete Phase 2 (physical crates/* migration). Do not merge runtime into taskfmt. Do not restore harness fingerprint or host progress-init.
```

This file is the **single authoritative specification**. Every requirement, locked decision,
file mapping, and verification step from design through implementation lives here.

---

## 1. Context and rationale

### Problem

The harness ships as one Rust crate (`harness/`) with mixed concerns:

- **In-container validation** (lint, verify, progress) — baked into agent images for agents.
- **In-container runtime** (PID 1, DinD, postgres prereqs, agent supervisor) — not validation.
- **Host orchestration** (docker dispatch, gate records, promote, experiment loop) — never in image.

Operators and agents need **clear binary boundaries**. The host builds all artifacts; there is
**no host/image harness fingerprint parity check** (removed in commit `3f50d26`).

### Goal

Split into a Cargo workspace:

| Crate / binary | Role |
|----------------|------|
| **`taskfmt-core`** | Shared library: lint, gate, progress, parsers |
| **`taskfmt`** | In-container validation binary |
| **`taskfmt-runtime`** | In-container boot/supervisor binary (**never merged into taskfmt**) |
| **`taskfmt-host`** | Host operator binary |
| **`harness/`** | Docker images, testdata, integration tests (assets, not domain logic) |

### Non-goals (do not implement)

- Restoring `project`, `group`, `study`, `selfhost`, `task-monitor` CLI (removed earlier).
- Restoring harness source **fingerprint** CLI or `require_image_fingerprint_match`.
- Renaming **workspace tamper** detection (`workspace_fingerprint`, `gate-fingerprint.json` in `run.rs`) — keep as-is.
- Merging `taskfmt-runtime` into `taskfmt`.

---

## 2. Locked decisions

| # | Topic | Decision |
|---|-------|----------|
| 1 | In-container binaries | **Split** — `taskfmt` + `taskfmt-runtime`; **never merge** |
| 2 | Progress seeding | **Runtime entrypoint** runs `gosu agent taskfmt init` if `/progress/progress.md` missing |
| 3 | Host seeds progress | **No** — removed from `taskfmt-host run`; entrypoint owns init |
| 4 | Host `progress-init` | **Removed** from host CLI entirely |
| 5 | In-container init name | **`taskfmt init`** (not `progress-init`) |
| 6 | Init semantics | **Once only** — `create_new`; exit **64** if file exists |
| 7 | In-container progress query | **`taskfmt status`** (plain default + `--json`); not `progress show/check` |
| 8 | Host run completion | **`taskfmt-host status`** — herdr/transcripts/container (unchanged, different from in-container `status`) |
| 9 | Host `lint` | **Keep** on `taskfmt-host` (manifest + task selection) |
| 10 | Library crate name | **`taskfmt-core`** |
| 11 | Asset home | **`harness/`** keeps `images/`, `testdata/`, `tests/` |
| 12 | Host builds everything | Host runs `build-images`; no double-verify digest at dispatch |
| 13 | Pre-dispatch image check | **`image_prerequisites()`** only (postgres tar + inner docker) — keep |
| 14 | Gate authority | **Host `gate`** is promotion authority; agent runs in-container **`verify`** |
| 15 | Harness fingerprint | **Removed** — do not reintroduce |

---

## 3. Target workspace layout

```
task-format/
  Cargo.toml                          # [workspace] members (see §10)
  docs/crate-split.md                 # this file
  crates/
    taskfmt-core/                     # lib
      Cargo.toml
      src/
        lib.rs
        acceptance.rs, gate.rs, hash.rs, lint.rs, progress.rs, redact.rs,
        taskfile.rs, verifycfg.rs, executioncfg.rs, selfcheck.rs, selftest.rs
        ops/mod.rs, ops/git.rs, ops/ignore.rs   # gate scope only
    taskfmt/                          # validation bin
      Cargo.toml
      src/main.rs
      src/cli.rs                      # init, status, lint, verify
      src/cmds/                       # init, progress_status, lint (paths), verify
    taskfmt-runtime/                  # runtime bin
      Cargo.toml
      src/main.rs
      src/cli.rs                      # container-entrypoint, prereqs, agent-launch, codex-login
      src/cmds/                       # container_entrypoint, agent_launch
      src/ops/signals.rs
    taskfmt-host/                     # host bin
      Cargo.toml
      src/main.rs
      src/cli.rs                      # host subcommands (no progress-init, no fingerprint)
      src/cmds/                       # run, experiment, gate, promote, status, …
      src/config.rs, runstate.rs, selection.rs, interactive.rs
      src/ops/                        # docker, container, images, gh, git, herdr, transcript, op
  harness/                            # assets + integration tests
    images/
    testdata/
    tests/
    checks/                           # if any remain
```

**Install (final):**

```sh
cargo install --path crates/taskfmt-host --locked --bin taskfmt-host
cargo install --path crates/taskfmt --locked --bin taskfmt
cargo install --path crates/taskfmt-runtime --locked --bin taskfmt-runtime
```

**Docker image (`harness/images/taskfmt/Dockerfile`):** build **both** `taskfmt` and
`taskfmt-runtime`; copy both into `harness-base` (`harness/images/base/Dockerfile`).

**Entrypoint:** `ENTRYPOINT ["/usr/local/bin/taskfmt-runtime", "container-entrypoint"]`

---

## 4. Binary specifications

### 4.1 `taskfmt` — in-container validation

**Path in image:** `/usr/local/bin/taskfmt`

| Subcommand | Behavior | Exit codes |
|------------|----------|------------|
| **`init`** | Create progress file from task README via `progress::generate` + `create_new` | 0 success; **64** if output exists; 1 other errors |
| **`status`** | Load `ProgressFile` against task checklist; print state | 0 valid; 1 missing/invalid |
| **`lint`** | Lint task package(s); default path `$TASKFMT_TASK_DIR` / `/task` | 0/1 |
| **`verify`** | Full gate via `gate::run`; last line `DONE` required | 0 pass; 1 fail |

**Must NOT expose:** `container-entrypoint`, `prereqs`, `agent-launch`, `codex-login`, `run`,
`experiment`, `gate`, `build-images`, `selfcheck`, `progress-init`, `fingerprint`.

#### `init` detail

- **Callable only once** per progress file path (atomic `create_new`).
- Error when exists: `taskfmt init: <path> already exists; init runs only once`
- Default output: `$PROGRESS_FILE` → `/progress/progress.md`
- Default task dir: `$TASKFMT_TASK_DIR` → `/task`
- Constant: `init::EXIT_ALREADY_INITIALIZED = 64`

#### `status` detail

Plain stdout (default):

```text
task=TASK-042 state=IN_PROGRESS current=1.2 latest_event=3
```

With `--json`:

```json
{"task":"TASK-042","state":"IN_PROGRESS","current":"1.2","latest_event":3,"completed":["1.1"]}
```

Does **not** run scope diff, `verify.toml` checks, or gate — only progress grammar validation.

#### Environment

| Variable | Used by | Default |
|----------|---------|---------|
| `TASKFMT_TASK_DIR` | init, status, lint, verify | `/task` |
| `TASKFMT_ROOT` | verify | git toplevel / cwd / `/work` |
| `TASKFMT_BASE` | verify scope | env → verify.toml → `baseline` |
| `PROGRESS_FILE` | init, status, verify | `/progress/progress.md` |

---

### 4.2 `taskfmt-runtime` — in-container boot

**Path in image:** `/usr/local/bin/taskfmt-runtime`

| Subcommand | Handler source (current `harness/src/`) |
|------------|----------------------------------------|
| `container-entrypoint` | `cmds/container_entrypoint.rs` |
| `prereqs` | `container_entrypoint::prereqs_only()` |
| `agent-launch` | `cmds/agent_launch.rs` |
| `codex-login` | `container_entrypoint::codex_login()` |

**Entrypoint sequence (PID 1, root):**

1. Start inner dockerd (DinD)
2. Seed agent auth (Codex/Cursor/Claude per image)
3. `chown agent` on `/work`, `/out`, `/agent-home`, **`/progress`**
4. **If `/progress/progress.md` missing:** `gosu agent taskfmt init` (fail → park container)
5. Run `prereqs` (postgres tar, seeds)
6. `PREREQ_ONLY=1` → park (docker itest)
7. `gosu agent taskfmt-runtime agent-launch` (replace PID 1 chain)

**Must NOT expose:** `init`, `status`, `lint`, `verify`, or any host subcommand.

---

### 4.3 `taskfmt-host` — host orchestration

**Default binary for `cargo run`** in workspace.

| Subcommand | Purpose |
|------------|---------|
| `lint` | Lint tasks under `experiment.toml` `tasks_dir` |
| `selftest` | Bundled corpus proof |
| `selfcheck` | D13 gate polarity on host workspace |
| `build-images` / `preload` | Image pipeline |
| `repo` | GitHub lifecycle |
| `run` | Dispatch one task to one container |
| `gate` | Host gate on frozen worktree |
| `promote` | Push promoted tree |
| **`status`** | **Run completion** (herdr, transcript, container) — NOT progress file |
| `attach` / `ps` | Container observe/control |
| `experiment` | Batch run → gate → promote |

**Removed permanently:** `progress-init`, `fingerprint`, `verify` (agents use in-container `taskfmt verify`).

**Pre-dispatch:** `docker::image_prerequisites(&profile.image)` in `run.rs` before run dir creation.

---

## 5. Two different commands named `status`

| | **`taskfmt status`** | **`taskfmt-host status <run-id>`** |
|--|----------------------|-----------------------------------|
| **Binary** | In-container validation | Host orchestration |
| **Reads** | `/progress/progress.md` | herdr, transcript, docker, manifest |
| **Answers** | Is progress file valid? What's `state`/`current`? | Has the agent run finished? Why? |
| **Module** | `cmds/progress_status.rs` | `cmds/status.rs` |
| **Used by** | Agent during work | Operator / `run --wait` / pre-gate |

---

## 6. Agent protocol

Sources to update and keep aligned:

- `reference/task-template/AGENTS.md`
- `reference/task-template/CLAUDE.md`
- `harness/src/task-prompt.md`

**Protocol steps:**

1. Read `/task/README.md`. On resume, read `/progress/progress.md` — **never** call `taskfmt init` again.
2. Work checklist leaves; append valid progress events only.
3. After progress edits: **`taskfmt status`**
4. Code iteration: **`taskfmt verify --progress ""`** from `/work` until exit 0 + `DONE`
5. Append terminal progress event (`state: DONE`)
6. **`taskfmt status`** — confirm `state=DONE`
7. **`taskfmt verify`** (full, progress check enabled) — completion evidence

**Turn signal (unchanged):** `GOAL_PROGRESS task=… state=… current=…`

---

## 7. Run lifecycle

```
taskfmt-host run
  ├─ load experiment.toml
  ├─ image_prerequisites(image)
  ├─ host lint (optional selfcheck)
  ├─ clone repo, snapshot /task, mount /progress (empty dir OK)
  ├─ docker run → taskfmt-runtime container-entrypoint
  │     ├─ taskfmt init (if /progress/progress.md missing)
  │     ├─ prereqs
  │     └─ taskfmt-runtime agent-launch
  ├─ agent loop: taskfmt status, taskfmt verify
  ├─ taskfmt-host status (run terminal?)
  ├─ quiesce + taskfmt-host gate (gate::run on host, frozen tree)
  └─ taskfmt-host promote (if gate pass)
```

Host **gate** calls `taskfmt_core::gate::run()` locally — same engine as in-container **`verify`**.

---

## 8. Module migration map

Current sources live under `harness/src/`. Target ownership:

### `taskfmt-core` (library)

| File | Notes |
|------|-------|
| `acceptance.rs` | |
| `gate.rs` | Shared gate engine |
| `hash.rs` | Evidence digests (not harness fingerprint) |
| `lint.rs` | |
| `progress.rs` | progress/v1 |
| `redact.rs` | |
| `taskfile.rs` | |
| `verifycfg.rs` | |
| `executioncfg.rs` | Lint validation only |
| `selfcheck.rs` | Library; CLI on host |
| `selftest.rs` | Library; CLI on host |
| `ops/mod.rs`, `ops/git.rs`, `ops/ignore.rs` | Gate scope / git diff for verify |

**Lib name for tests:** publish as `[lib] name = "taskfmt"` inside `taskfmt-core` package so
existing integration tests can `use taskfmt::…` with a dependency alias, OR update all tests to
`taskfmt_core::` — pick one approach and apply consistently.

### `taskfmt` (validation binary)

| File | Notes |
|------|-------|
| `cli/container.rs` → `cli.rs` | init, status, lint, verify |
| `cmds/init.rs` | |
| `cmds/progress_status.rs` | |
| `cmds/lint.rs` | `run_paths()` only |
| `cmds/verify.rs` | |
| `bin/taskfmt.rs` → `main.rs` | calls `dispatch_validation()` |

### `taskfmt-runtime` (runtime binary)

| File | Notes |
|------|-------|
| `cli/runtime.rs` → `cli.rs` | |
| `cmds/container_entrypoint.rs` | includes init-if-missing |
| `cmds/agent_launch.rs` | |
| `ops/signals.rs` | |
| `bin/taskfmt-runtime.rs` → `main.rs` | calls `dispatch_runtime()` |

### `taskfmt-host` (host binary)

| File | Notes |
|------|-------|
| `cli/host.rs` → `cli.rs` | no ProgressInit, no Fingerprint |
| `config.rs`, `runstate.rs`, `selection.rs`, `interactive.rs` | |
| `cmds/run.rs`, `experiment.rs`, `gate.rs`, `promote.rs`, `status.rs`, `attach.rs`, `ps.rs`, … | |
| `cmds/lint.rs` | `run()` with manifest |
| `ops/docker.rs`, `ops/container.rs`, `ops/images.rs`, `ops/gh.rs`, `ops/herdr.rs`, `ops/transcript.rs`, `ops/op.rs` | |
| `bin/taskfmt-host.rs` → `main.rs` | |

### `harness/` (assets + tests)

| Path | Role |
|------|------|
| `harness/images/` | Dockerfiles |
| `harness/testdata/` | Fixtures |
| `harness/tests/` | Integration tests (depend on workspace crates) |
| `harness/tests/run_docker_itest.sh` | Docker itest runner |
| `harness/build.rs` | Move to `taskfmt-core` — bakes `GIT_COMMIT_SHA` only (no fingerprint) |

### Deleted (do not restore)

| File | Reason |
|------|--------|
| `harness/src/fingerprint.rs` | Removed `3f50d26` |
| `harness/src/cmds/fingerprint.rs` | Removed `3f50d26` |
| `harness/checks/fingerprint_cli.rs` | Removed `3f50d26` |
| `harness/src/cmds/progress_init.rs` | Replaced by `init.rs` |

---

## 9. Implementation phases

### Phase 1 — Behavioral split (three binaries in `harness/`)

**Status:** Implemented on disk (may be uncommitted). Agent must **verify** every item.

| # | Task | Evidence |
|---|------|----------|
| 1.1 | Add `taskfmt init` | `harness/src/cmds/init.rs`, `cli/container.rs` |
| 1.2 | Add `taskfmt status` | `harness/src/cmds/progress_status.rs` |
| 1.3 | Add `taskfmt-runtime` binary | `harness/src/bin/taskfmt-runtime.rs`, `cli/runtime.rs` |
| 1.4 | Remove runtime cmds from `taskfmt` CLI | `cli/container.rs` has only init/status/lint/verify |
| 1.5 | Entrypoint calls `taskfmt init` if missing | `container_entrypoint.rs` |
| 1.6 | Entrypoint uses `taskfmt-runtime agent-launch` | `container_entrypoint.rs` |
| 1.7 | Remove host `progress-init` | `cli/host.rs` |
| 1.8 | Remove progress seed from `run.rs` | no `generate_and_write` in dispatch path |
| 1.9 | Update Dockerfiles | both bins; runtime ENTRYPOINT |
| 1.10 | Update tests | `cli_boundaries.rs`, `config_selection_interactive.rs` |
| 1.11 | Update agent docs | `AGENTS.md`, `CLAUDE.md`, `task-prompt.md` if needed |
| 1.12 | Root workspace stub | `Cargo.toml` with `members = ["harness"]` |

### Phase 2 — Physical workspace migration

| # | Task |
|---|------|
| 2.1 | Create `crates/taskfmt-core/`; move library modules per §8 |
| 2.2 | Create `crates/taskfmt/`; move validation bin + cmds |
| 2.3 | Create `crates/taskfmt-runtime/`; move runtime bin + cmds |
| 2.4 | Create `crates/taskfmt-host/`; move host bin + cmds + host modules |
| 2.5 | Update root `Cargo.toml` `members = ["crates/taskfmt-core", "crates/taskfmt", "crates/taskfmt-runtime", "crates/taskfmt-host", "harness"]` |
| 2.6 | Convert `harness/` to integration-test + assets crate OR keep as thin wrapper — tests must still run |
| 2.7 | Update `harness/images/taskfmt/Dockerfile` build context/paths for workspace |
| 2.8 | Update `harness/src/ops/images.rs` docker build paths if context changes |
| 2.9 | Update `README.md`, `harness/README.md` install paths to `crates/*` |
| 2.10 | Grep repo — zero stale `progress-init`, `fingerprint`, monolithic-only paths |

---

## 10. Workspace `Cargo.toml` (final target)

```toml
[workspace]
resolver = "2"
members = [
  "crates/taskfmt-core",
  "crates/taskfmt",
  "crates/taskfmt-runtime",
  "crates/taskfmt-host",
  "harness",
]

[workspace.package]
edition = "2024"
rust-version = "1.98"
license = "Apache-2.0"

[profile.release]
strip = true
lto = "thin"
```

Each binary crate sets `default-run` only on `taskfmt-host`.

---

## 11. Verification — definition of done

Run **all** checks after Phase 1 and again after Phase 2.

### 11.1 Build and static analysis

```sh
cd /path/to/task-format

# After Phase 2, use workspace root; during Phase 1, harness/ alone is OK:
cargo test --manifest-path harness/Cargo.toml
cargo clippy --manifest-path harness/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path harness/Cargo.toml -- --check

# Phase 2 final:
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

**Pass criteria:** all tests green; clippy no warnings; fmt clean.

### 11.2 CLI surface (help output)

```sh
cargo run --manifest-path harness/Cargo.toml --bin taskfmt -- --help
cargo run --manifest-path harness/Cargo.toml --bin taskfmt-runtime -- --help
cargo run --manifest-path harness/Cargo.toml --bin taskfmt-host -- --help
```

**Pass criteria:**

| Binary | Must include | Must NOT include |
|--------|--------------|------------------|
| `taskfmt` | `init`, `status`, `lint`, `verify` | `container-entrypoint`, `prereqs`, `agent-launch`, `run`, `gate`, `progress-init`, `fingerprint` |
| `taskfmt-runtime` | `container-entrypoint`, `prereqs`, `agent-launch`, `codex-login` | `init`, `status`, `lint`, `verify`, `run`, `gate` |
| `taskfmt-host` | `lint`, `run`, `gate`, `experiment`, `status`, … | `init`, `verify`, `progress-init`, `fingerprint`, `container-entrypoint` |

Automated: `harness/tests/cli_boundaries.rs` must pass.

### 11.3 `init` semantics

```sh
# In temp fixture or unit test:
# - first init → exit 0, file created
# - second init same path → exit 64
```

Automated: `init::tests::second_init_refuses_with_64` in `harness/src/cmds/init.rs`.

Manual/integration: entrypoint with empty `/progress` mount creates file once; second container
start with same mount does not overwrite (init not called when file exists).

### 11.4 `status` semantics

- Valid progress file → exit 0, prints `task=… state=…`
- Missing/malformed → exit 1
- `--json` emits valid JSON

### 11.5 Host run does not seed progress

Grep `harness/src/cmds/run.rs` (or `crates/taskfmt-host/src/cmds/run.rs` after Phase 2):

- **Must not** call `generate_and_write` or `progress_init` on dispatch path.
- **Must** call `image_prerequisites` before run dir.

### 11.6 Entrypoint

Grep `container_entrypoint.rs`:

- **Must** call `taskfmt init` (via gosu) when `/progress/progress.md` missing.
- **Must** exec `taskfmt-runtime agent-launch` (not `taskfmt agent-launch`).

### 11.7 Docker / image

```sh
taskfmt-host preload --auto
taskfmt-host build-images --agent all --auto
```

Inspect built image:

```sh
docker run --rm --entrypoint /usr/local/bin/taskfmt harness-base:latest --help
docker run --rm --entrypoint /usr/local/bin/taskfmt-runtime harness-base:latest --help
```

**Pass criteria:** both binaries exist; base ENTRYPOINT is `taskfmt-runtime container-entrypoint`.

Optional: `sh harness/tests/run_docker_itest.sh` (requires Docker).

### 11.8 Documentation grep

```sh
rg -i 'progress-init|HARNESS_FINGERPRINT|require_image_fingerprint|fingerprint_cli' .
rg 'taskfmt progress-init' .
```

**Pass criteria:** no matches except this file's historical notes or changelog.

Progress/init docs **must** say `taskfmt init` and `taskfmt status`.

### 11.9 Gate and promote unchanged

- `GateRecord::promotable()` does **not** require `harness_fingerprint` (removed `3f50d26`).
- Workspace tamper `gate-fingerprint.json` still written in gate path (`run.rs`).

Existing gate/promote tests must pass.

### 11.10 Test count baseline

Phase 1 baseline: **335+** tests passing in `harness/`. Phase 2 must not regress.

---

## 12. Files to update (checklist)

### Code (Phase 1 — verify present)

- [ ] `harness/src/cmds/init.rs`
- [ ] `harness/src/cmds/progress_status.rs`
- [ ] `harness/src/cli/container.rs`
- [ ] `harness/src/cli/runtime.rs`
- [ ] `harness/src/cli/host.rs` (no ProgressInit)
- [ ] `harness/src/cmds/mod.rs` (`dispatch_validation`, `dispatch_runtime`, `dispatch_host`)
- [ ] `harness/src/bin/taskfmt.rs`, `taskfmt-runtime.rs`, `taskfmt-host.rs`
- [ ] `harness/src/cmds/container_entrypoint.rs`
- [ ] `harness/src/cmds/run.rs` (no progress seed)
- [ ] `harness/Cargo.toml` (three bins)
- [ ] `harness/images/taskfmt/Dockerfile`, `images/base/Dockerfile`

### Code (Phase 2 — create/move)

- [ ] `crates/taskfmt-core/Cargo.toml`, `src/lib.rs`, …
- [ ] `crates/taskfmt/Cargo.toml`, `src/main.rs`, …
- [ ] `crates/taskfmt-runtime/Cargo.toml`, `src/main.rs`, …
- [ ] `crates/taskfmt-host/Cargo.toml`, `src/main.rs`, …
- [ ] Root `Cargo.toml` workspace members
- [ ] `harness/Cargo.toml` → test-only crate depending on workspace

### Tests

- [ ] `harness/tests/cli_boundaries.rs`
- [ ] `harness/tests/config_selection_interactive.rs`
- [ ] `harness/tests/gate_tamper_matrix.rs` (uses `init::generate_and_write`)
- [ ] All other `harness/tests/*.rs` compile against new crate paths

### Docs

- [ ] `docs/crate-split.md` (this file — update checklist when done)
- [ ] `README.md`
- [ ] `harness/README.md`
- [ ] `reference/task-template/AGENTS.md`
- [ ] `reference/task-template/CLAUDE.md`
- [ ] `harness/src/task-prompt.md`
- [ ] `docs/monitoring.md` (install/build paths if changed)

---

## 13. Suggested commit messages

**Phase 1 (if not yet committed):**

```text
feat: split validation and runtime binaries; add taskfmt init and status

Separate taskfmt (init/status/lint/verify) from taskfmt-runtime (entrypoint).
Entrypoint seeds progress via taskfmt init; remove host progress-init.
```

**Phase 2:**

```text
refactor: move harness into taskfmt workspace crates

Extract taskfmt-core, taskfmt, taskfmt-runtime, and taskfmt-host crates;
keep harness/ for images and integration tests.
```

---

## 14. Migration checklist (living)

- [x] Write this execution plan (`docs/crate-split.md`)
- [x] Phase 1 behavioral split (on disk — verify §11)
- [x] Phase 1 committed
- [x] Phase 2 physical `crates/*` migration
- [x] Phase 2 verification (§11) all green
- [x] Docs and checklist in §14 fully checked

---

## 15. Related files (quick index)

| Path | Purpose |
|------|---------|
| `docs/crate-split.md` | **This execution plan** |
| `harness/src/cmds/init.rs` | `taskfmt init` |
| `harness/src/cmds/progress_status.rs` | `taskfmt status` |
| `harness/src/cmds/status.rs` | `taskfmt-host status` (run completion) |
| `harness/src/gate.rs` | Shared gate engine |
| `harness/src/progress.rs` | progress/v1 grammar |
| `harness/src/ops/container.rs` | Mounts, docker run spec |
| `harness/src/ops/docker.rs` | `image_prerequisites()` |
| `harness/tests/cli_boundaries.rs` | Binary boundary tests |
| `reference/task-template/AGENTS.md` | Agent protocol |
| `harness/images/taskfmt/Dockerfile` | Builds both in-container binaries |
| `harness/images/base/Dockerfile` | Runtime ENTRYPOINT |
