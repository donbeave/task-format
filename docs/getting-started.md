# Getting started

This guide explains how to install the taskfmt binaries on your machine, verify that the
repository builds correctly, and run the bundled examples. It is written for operators and
contributors who want to try the harness locally before dispatching agent runs in Docker.

For the full operator reference (flags, safety model, agent profiles, and secrets), see
[harness/README.md](../harness/README.md). For research context and the trust model, see the
[root README](../README.md).

## What this repository ships

The Rust workspace builds **three command-line binaries**. Each has a distinct role:

| Binary | Crate | Role |
| --- | --- | --- |
| `taskfmt-host` | `crates/taskfmt-host` | Host operator CLI: lint tasks, dispatch runs, gate results, promote commits, run experiments, build images. |
| `taskfmt` | `crates/taskfmt` | In-container validation: seed progress, report status, lint packages, run the completion gate (`verify`). |
| `taskfmt-runtime` | `crates/taskfmt-runtime` | In-container boot: entrypoint, prerequisites, agent launch. |

Shared library code lives in `crates/taskfmt-core` (import name `taskfmt`). Docker assets,
integration tests, and bundled fixtures live under `harness/`.

You do **not** need Docker to lint task packages or run the bundled selftest. Docker is required
only for `run`, `experiment`, `build-images`, and `preload`.

## Prerequisites

| Requirement | Needed for |
| --- | --- |
| **Rust 1.98+** (`rustup`) | Building and installing all binaries |
| **Git** | Cloning the repo; scope checks inside `verify` |
| **Docker** (daemon running) | Dispatching agent runs, building harness images |
| **Agent credentials** | Live runs only (see [Agent setup](#agent-setup-for-live-runs)) |

Clone the repository and work from its root for every command below:

```sh
git clone https://github.com/donbeave/task-format.git
cd task-format
```

## Install the binaries

Choose one of the following approaches.

### Option A — Install to `~/.cargo/bin` (recommended)

From the repository root:

```sh
cargo install --path crates/taskfmt-host --locked --bin taskfmt-host
cargo install --path crates/taskfmt --locked --bin taskfmt
cargo install --path crates/taskfmt-runtime --locked --bin taskfmt-runtime
```

Ensure `~/.cargo/bin` is on your `PATH`, then confirm:

```sh
taskfmt-host --version
taskfmt --version
taskfmt-runtime --version
```

Re-run the install commands after changing Rust code so your PATH binaries stay in sync with the
repo.

### Option B — Build once, run from `target/debug`

```sh
cargo build --workspace
```

Run without installing:

```sh
./target/debug/taskfmt-host --help
./target/debug/taskfmt lint harness/testdata/example
```

### Option C — No install: `cargo run`

Useful while iterating on a single crate:

```sh
cargo run --bin taskfmt-host -- lint TASK-001
cargo run --bin taskfmt -- lint harness/testdata/example
```

## Verify the install (no Docker)

These checks use only the Rust toolchain and files already in the repository.

### 1. Host selftest (bundled corpus)

`selftest` exercises lint rules, progress init, and gate behavior on an embedded corpus. It needs
no config file and no secrets:

```sh
taskfmt-host selftest
```

Expected last line:

```text
SELFTEST PASS
```

### 2. Lint an experiment task

Task packages for the pgtui experiment live under `experiments/tasks/`. Lint one by ID (resolved
via `experiment.toml` → `paths.tasks_dir`):

```sh
taskfmt-host lint TASK-001
```

Expected output shape:

```text
PACKAGE .../experiments/tasks/TASK-001/README.md
SUMMARY errors=0 warnings=0
LINT PASS
```

Lint every configured task:

```sh
taskfmt-host lint
```

Machine-readable output:

```sh
taskfmt-host lint --json
```

### 3. Lint the minimal bundled example

`harness/testdata/example/` is a small, self-contained task package used by integration tests.
It validates the `task/v5` README schema and `verify/v2` layout without requiring a live codebase:

```sh
taskfmt lint harness/testdata/example
```

You can also lint by pointing at the README directly:

```sh
taskfmt lint harness/testdata/example/README.md
```

### 4. Progress init and status (agent workflow)

Inside a container, agents coordinate through `/progress/progress.md`. You can simulate that
locally with the bundled example:

```sh
PROGRESS=$(mktemp)
taskfmt init --task-dir harness/testdata/example --out "$PROGRESS"
taskfmt status --task-dir harness/testdata/example --progress "$PROGRESS"
```

Example status line:

```text
task=TASK-042 state=IN_PROGRESS current=1.1 latest_event=1
```

`init` is create-once: running it again against the same output file exits with code 64.

### 5. Run the Rust test suite

From the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The workspace currently runs **338** integration and unit tests across `crates/*` and
`harness/tests/`.

Optional Docker integration gate (requires built images):

```sh
taskfmt-host build-images --agent all --auto
sh harness/tests/run_docker_itest.sh
```

## Examples in this repository

Use these paths when exploring or authoring task packages:

| Path | Purpose |
| --- | --- |
| [`harness/testdata/example/`](../harness/testdata/example/) | Minimal lint corpus (`TASK-042`); good first lint target. |
| [`harness/testdata/verify-template.toml`](../harness/testdata/verify-template.toml) | Parseable `verify/v2` skeleton. |
| [`harness/testdata/execution-template.toml`](../harness/testdata/execution-template.toml) | Parseable `execution/v1` skeleton. |
| [`reference/task-template/`](../reference/task-template/) | Canonical template for new packages (`README.md`, `AGENTS.md`, `verify.toml`). |
| [`experiments/tasks/TASK-001`](../experiments/tasks/TASK-001/) … `TASK-007` | Full experiment corpus (pgtui vertical slices). |
| [`experiments/fixtures/`](../experiments/fixtures/) | Shared deterministic seed data for runs. |
| [`projects/demo/pgtui/`](../projects/demo/pgtui/) | Filesystem catalog mirror of the demo tasks (browser monitor). |
| [`experiment.toml`](../experiment.toml) | Paths, images, runtime limits, and agent profiles. |

### Read a task package

Every task package contains at least:

- `README.md` — goal, requirements, acceptance criteria, checklist (`task/v5` schema).
- `verify.toml` — checks, writable paths, gate commands (`verify/v2` schema).
- `AGENTS.md` — agent execution protocol (in the template and live packages).
- `trusted/` — planner-owned tests and fixtures (experiment tasks only).

Open the bundled example README and matching verify config:

```sh
sed -n '1,40p' harness/testdata/example/README.md
cat harness/testdata/example/verify.toml
```

Compare with a live experiment task:

```sh
sed -n '1,40p' experiments/tasks/TASK-001/README.md
head -30 experiments/tasks/TASK-001/verify.toml
```

## Configuration

Commands that need repository layout read `experiment.toml`. Resolution order:

1. `--config /path/to/experiment.toml`
2. Environment variable `TASKFMT_CONFIG`
3. Nearest `experiment.toml` found by walking upward from the current directory

When you run commands from the cloned repository root, the bundled
[`experiment.toml`](../experiment.toml) is picked up automatically. Paths inside it are relative
to that file’s directory.

## Agent setup for live runs

Skip this section if you only lint packages or run `selftest`.

### First-time Docker setup

```sh
taskfmt-host preload --auto
taskfmt-host build-images --agent all --auto
```

`build-images --agent all` builds, in order: `harness-taskfmt`, `harness-base`, then agent
layers for Claude, Codex, and Cursor. Re-run after changing harness Rust code.

### Claude with GLM-5.3-Flash (`zai-flash`, default profile)

Create a local token file:

```sh
mkdir -p ~/.config/taskfmt
chmod 700 ~/.config/taskfmt
$EDITOR ~/.config/taskfmt/zai-flash.token   # paste Z.ai API token only
chmod 600 ~/.config/taskfmt/zai-flash.token
```

Build the Claude image, then dispatch:

```sh
taskfmt-host build-images --agent claude --auto
taskfmt-host lint TASK-001
taskfmt-host run --task TASK-001 --repo <repository-url> --agent zai-flash --wait
```

### Codex (`codex-default`)

Authenticate on the host first:

```sh
codex --login
taskfmt-host run --task TASK-001 --repo <repository-url> --agent codex-default --wait
```

See [harness/README.md](../harness/README.md) for Cursor, Codex-via-Z.ai, Kimi profiles, and
secret reference formats (`file://…`, `op://…`).

## Run one task end to end

Typical operator flow after setup:

```sh
# 1. Validate the task package
taskfmt-host lint TASK-001

# 2. Dispatch to a fresh container (--wait runs the host gate when the agent finishes)
taskfmt-host run --task TASK-001 --repo <repository-url> --agent codex-default --wait

# 3. Promote only after a passing gate (run-id printed by run/status)
taskfmt-host promote <run-id> --auto
```

Without `--repo`, `run` creates a disposable private GitHub repository after confirmation. Omit
`--wait` to return immediately; then follow with:

```sh
taskfmt-host status <run-id> --wait
taskfmt-host gate <run-id> --auto
taskfmt-host promote <run-id> --auto
```

Run records and evidence are written under `experiments/runs/` (gitignored).

### Run an ordered experiment series

```sh
taskfmt-host experiment \
  --tasks 1-3 \
  --repo <repository-url> \
  --agent zai-flash \
  --auto
```

Selections include `all`, ranges like `1-3,5`, and explicit IDs like `TASK-002..TASK-004`.
Resume after interruption:

```sh
taskfmt-host experiment --resume <experiment-id> --tasks all --auto
```

### Inspect runs

```sh
taskfmt-host ps
taskfmt-host status <run-id>
taskfmt-host attach <run-id>    # detach with ctrl+b q, not ctrl+c
```

## Optional: gate selfcheck (D13)

Before dispatching a new task package, you can prove its gate fails on the untouched baseline
and passes on a reference solution. The workspace is copied to a scratch directory; the original
is never mutated:

```sh
taskfmt-host selfcheck experiments/tasks/TASK-001 <git-checkout-at-base> \
  --reference <reference-tree-or-patch> \
  --auto
```

Exit code 0 means `SELFCHECK RESULT PASS`. This is optional and uses the host toolchain; it does
not require Docker.

## Command cheat sheet

| Goal | Command |
| --- | --- |
| Install all binaries | Three `cargo install --path crates/...` lines above |
| Prove harness logic locally | `taskfmt-host selftest` |
| Lint one experiment task | `taskfmt-host lint TASK-001` |
| Lint bundled example | `taskfmt lint harness/testdata/example` |
| Seed progress file | `taskfmt init --task-dir <pkg> --out <file>` |
| Read progress state | `taskfmt status --task-dir <pkg> --progress <file>` |
| Run completion gate in workspace | `taskfmt verify --task-dir <pkg> --root <repo>` |
| Build Docker images | `taskfmt-host build-images --agent all --auto` |
| Dispatch one task | `taskfmt-host run --task TASK-001 --repo <url> --wait` |
| Run test suite | `cargo test --workspace` |

## Where to go next

- [harness/README.md](../harness/README.md) — complete CLI reference, safety model, secrets, per-task `execution.toml`.
- [reference/task-template/AGENTS.md](../reference/task-template/AGENTS.md) — protocol agents follow inside containers.
- [docs/monitoring.md](monitoring.md) — browser monitor for the `projects/` catalog.
- [docs/crate-split.md](crate-split.md) — workspace layout and binary responsibilities.
