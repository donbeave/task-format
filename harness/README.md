# Harness

`taskfmt-host` runs one bounded coding task in the caller-provided execution boundary. It gates a frozen candidate tree and can promote that exact tree.

AI agents can change a repository quickly. A run record is promotion evidence only after a passing host gate has recorded an immutable candidate tree, its expected parent, and verifier evidence.

The Rust workspace ships three binaries from `crates/*` (see [`docs/crate-split.md`](../docs/crate-split.md)); this directory holds Docker images, testdata, and integration tests:

- **`taskfmt-host`** — operator CLI on the host: lint, dispatch, gate, promote, experiment, images.
- **`taskfmt`** — in-container validation: `init`, `status`, `lint`, `verify`.
- **`taskfmt-runtime`** — in-container boot: `container-entrypoint`, `prereqs`, `agent-launch`, `codex-login`.

Repository-level settings live in [`experiment.toml`](../experiment.toml).

## Lifecycle

For each task, `taskfmt-host`:

1. Lints the task package before dispatch.
2. Creates a fresh workspace from the experiment repository's `main` branch.
3. Adds planner-owned trusted verification material to that workspace.
4. Starts one headed agent container with a writable `/work` workspace and read-only task instructions.
5. Records run state under `experiments/runs/`.
6. Stops the executor, stages one complete candidate tree, and runs the host gate against that tree.
7. Records gate evidence, tree, and expected parent; `promote` creates and pushes only that recorded tree with an expected-parent lease.

The harness supports Claude, Codex, and Cursor agent profiles. Images are built from [`images/`](images/); profile, runtime, repository, and path settings belong in [`experiment.toml`](../experiment.toml).

## Safety model

- **Task package** — planner-owned instructions and verifier inputs. Start from the [task template](../reference/task-template/README.md); configured packages are under [`experiments/tasks/`](../experiments/tasks/).
- **Trusted material** — tests, fixtures, support code, or other verifier inputs supplied by the planner. It is added to the run workspace before the agent starts and is outside the agent's allowed output paths.
- **Run** — one task, one fresh container, one recorded workspace. Its record is `experiments/runs/<id>/manifest.json`.
- **Gate** — `taskfmt-host gate` re-runs verification against the frozen candidate at the caller-provided boundary. In-container agents use `taskfmt verify`; the host gate record includes the candidate tree, parent, verifier evidence, and verdict.
- **Promotion** — creates a commit from the recorded passing tree and parent, then pushes it with an expected-parent lease. It refuses failed, incomplete, changed, or stale-parent records.

Task-package Markdown, the launch prompt, and bundled task fixtures are executable inputs. Do not casually rewrite them while changing operator documentation.

## Install and first-time setup

See **[docs/getting-started.md](../docs/getting-started.md)** for install options (`cargo install`,
`cargo build`, or `cargo run`), local verification without Docker, and a map of bundled examples.

Run from the repository root. Docker must be running.

```sh
cargo install --path crates/taskfmt-host --locked --bin taskfmt-host
cargo install --path crates/taskfmt --locked --bin taskfmt
cargo install --path crates/taskfmt-runtime --locked --bin taskfmt-runtime
taskfmt-host preload --auto
taskfmt-host build-images --agent all --auto
```

`build-images --agent all` builds, in order:

1. `harness-taskfmt:latest` — the Rust `taskfmt` binary;
2. `harness-base:latest` — Debian Trixie, Node.js 24 LTS, Rust 1.98, Herdr 0.8.2, PostgreSQL client 18, and runtime tools;
3. `harness-claude:latest` — Claude Code 2.1.261;
4. `harness-codex:latest` — Codex CLI 0.153.4;
5. `harness-cursor:latest` — Cursor Agent CLI 2026.09.10-fd3934a.

Use `--agent claude`, `--agent codex`, or `--agent cursor` to rebuild one agent layer. Use `--no-cache` when
refreshing moving base layers. `preload` uses the digest in
`images/preload/postgres.digest` and saves the matching PostgreSQL 18 image for the inner Docker
daemon.

Image building and runtime selection are separate. Build both images once, then choose a profile
per run.

### Claude with GLM-5.3-Flash

The `zai-flash` profile uses Claude Code through Z.ai's Anthropic-compatible API. The API key
is referenced from 1Password (`op://ChainArgos/Z.ai/Test`) or a local file:

```sh
mkdir -p ~/.config/taskfmt
chmod 700 ~/.config/taskfmt
$EDITOR ~/.config/taskfmt/zai-flash.token
chmod 600 ~/.config/taskfmt/zai-flash.token
```

In `experiment.toml`, set `ANTHROPIC_AUTH_TOKEN = "op://ChainArgos/Z.ai/Test"` or
`"file://zai-flash.token"`.

Run with `--agent zai-flash`, or omit `--agent` because it is the configured default:

```sh
taskfmt-host run --task TASK-001 --repo <repository-url> --agent zai-flash --wait
```

### Codex

Authenticate Codex on the host first. The configured profile uses `auth = "host"`, so the harness
binds the host's `CODEX_HOME/auth.json` (or `~/.codex/auth.json`) read-only to a staging path,
then copies it into an isolated container-only `CODEX_HOME` with container-user ownership. The
host file and run artifacts are not mutated, and no browser callback is needed inside Docker:

```sh
codex --login
```

The mount is explicit because it gives the fully privileged agent process access to the host
Codex credential. Use API-key auth instead for untrusted tasks by removing `auth = "host"` and
configuring `OPENAI_API_KEY` through `env_secret`.

Use the `codex-default` profile and the image built by `--agent all`:

```sh
taskfmt-host run --task TASK-001 --repo <repository-url> --agent codex-default --wait
```

## Run one task

Lint first, then dispatch to a fresh container:

```sh
taskfmt-host lint TASK-001
taskfmt-host run --task TASK-001 --repo <repository-url> --agent codex-default --wait
```

`--wait` waits for the agent, runs the host gate, and returns success only for a passing goal.
Promotion remains explicit:

```sh
taskfmt-host promote <run-id> --auto
```

To return immediately, omit `--wait`, then run:

```sh
taskfmt-host status <run-id> --wait
taskfmt-host gate <run-id> --auto
taskfmt-host promote <run-id> --auto
```

Without `--repo`, `run` creates a disposable private repository after confirmation. `--exp <id>`
records the run inside an experiment state file.

## Run experiments

`experiment` performs repository setup, dispatch, host gating, and promotion for each selected task
in order. It stops on the first failed or blocked task.

Run a range against an existing repository:

```sh
taskfmt-host experiment \
  --tasks 1-3 \
  --repo <repository-url> \
  --agent zai-flash \
  --auto
```

Run every configured task in one command:

```sh
taskfmt-host experiment \
  --tasks all \
  --repo <repository-url> \
  --agent zai-flash \
  --auto
```

Omit `--repo` to create a disposable private repository. Add
`--proof-corpus /path/to/pgtui-proof.git` to preflight the immutable baseline/reference corpus
before creating or mutating the runtime repository. The corpus path is recorded with the
experiment; resume reuses it and rejects a different path. Valid selections include `all`,
`1-3,5`, `TASK-002..TASK-004`, and `TASK-101`. Resume an interrupted experiment with its
recorded ID:

```sh
taskfmt-host experiment --resume <experiment-id> --tasks all --agent zai-flash --auto
```

Use `--selfcheck` to run the D13 gate selfcheck before each dispatch. It refuses to dispatch when
the selfcheck fails or has no verdict.

Mutating commands prompt for confirmation. `--auto` and `--yes` skip prompts; a non-interactive
shell requires one of them. Read-only commands do not prompt.

## Follow and inspect runs

```sh
taskfmt-host ps
taskfmt-host ps --json
taskfmt-host status <run-id>
taskfmt-host status <run-id> --wait
taskfmt-host attach <run-id>
```

`<run-id>` may be the run ID, `harness-<run-id>` container name, run directory, or its
`manifest.json`. `attach` reconnects to the live agent TUI; detach with `ctrl+b q`, never `ctrl+c`.
Run records and evidence live under `experiments/runs/`.

`status` prints one JSON line (run state, completion evidence, and a `progress` summary), then the
task checklist read from the run's `progress/progress.md`: `[x]` done, `[>]` in progress, `[!]`
failed or blocked, `[ ]` pending, with the active leaf marked `<- in progress`. With `--wait` the
checklist is reprinted whenever it moves. It is coordination evidence only — the gate decides
completion.

```text
progress: IN_PROGRESS  done 2/5  current 2.2  latest_event 7
[x] 1 Bootstrap workspace.
    [x] 1.1 Establish the workspace and pins for R-001; prove AC-001 via CHK-001.
    [x] 1.2 Configure member and toolchain for R-002 and R-003; prove AC-002 via CHK-002.
[>] 2 Preserve bootstrap behavior.
    [!] 2.1 Keep render and stub contracts for R-004 and R-005; prove AC-003 via CHK-003.  <- failed, not retried
    [>] 2.2 Keep scope protected for R-006 and R-007; prove AC-004 via CHK-004.  <- in progress
[ ] 3 Complete package verification.
    [ ] 3.1 Complete the package gate for R-006 and R-007; prove AC-005 via CHK-005.
```

## Task validation and gates

Validate task packages before dispatch:

```sh
taskfmt-host lint TASK-001
taskfmt-host lint --json
```

In-container progress (agents): `taskfmt init` (once), `taskfmt status`, then `taskfmt verify`.
The runtime entrypoint calls `taskfmt init` when `/progress/progress.md` is missing. `taskfmt
status` prints the derived header (`task= state= current= latest_event= done=`) followed by the
same per-item checklist as `taskfmt-host status`; `--json` carries it under `checklist`.

Re-run the host gate for a dispatched run:

```sh
taskfmt-host gate <run-id> --auto
```

The gate passes only when the declared checks pass and the final stdout line is `DONE`. It also
checks expected results, writable paths, scope, and progress. Agents prove completion in-container
with `taskfmt verify` (see the task template); the host does not expose `verify` on
`taskfmt-host`.

Prove a task gate distinguishes the untouched base from a reference solution:

```sh
taskfmt-host selfcheck <task-directory> <base-workspace> \
  --reference <reference-directory> \
  --auto
```

The base workspace is not mutated; selfcheck uses a scratch copy under `TMPDIR`. It is optional
because it runs focused checks with the host toolchain.

## Configuration and secrets

Commands resolve the manifest in this order:

1. `--config <path>`;
2. `TASKFMT_CONFIG`;
3. the nearest `experiment.toml` found by walking upward from the current directory.

Relative paths resolve from the manifest directory. The manifest controls task/run/fixture paths,
image names, runtime limits, GitHub repository defaults, and agent profiles. In this repository the
default profile is `zai-flash`; `codex-default` is also available.

Profile secrets are references, not values. They resolve only at dispatch, pass through a temporary
mode-0600 environment file, and are redacted from output and records.

### Per-task dispatch (`execution.toml`)

Each task package may optionally declare which agent profile, model, and effort level runs it.
Auth and secrets stay in `experiment.toml` profiles only; `execution.toml` names dispatch intent,
not credentials. When the file is absent, dispatch uses the experiment default profile unchanged.

Schema `execution/v1`:

```toml
schema = "execution/v1"
profile = "codex-kimi"   # required when the file exists
model = "kimi-k3"        # optional
effort = "high"          # optional: low | medium | high | max
```

Precedence (one resolver for `run`, `experiment`, and lint cross-checks):

| Field | Order (highest first) |
|-------|------------------------|
| profile | CLI `--agent` > `execution.toml` profile > `[agents.default].profile` |
| model | CLI `--model` > `execution.toml` model > profile.model |
| effort | CLI `--effort` > `execution.toml` effort > profile.effort |

`taskfmt-host experiment` accepts optional `--model` and `--effort` to override every selected task;
`--agent` overrides all tasks as well. The confirmation plan shows the resolved profile, model,
and effort per task. Task-package verification covers `README.md` and `verify.toml` only;
`execution.toml` does not affect those checks.

See [`reference/task-template/execution.toml`](../reference/task-template/execution.toml) for a
commented template and [`harness/testdata/execution-template.toml`](testdata/execution-template.toml)
for a parseable example.

## Inspection and repository lifecycle

Manage disposable GitHub repositories explicitly when needed:

```sh
taskfmt-host repo create --auto
taskfmt-host repo delete --name <repository-name> --auto
```

### Cursor

Cursor uses the same login as your host `agent` CLI. Run `agent login` on the host first.
On macOS the harness reads your Keychain session at dispatch time; on Linux it mounts
`~/.config/cursor/auth.json` (or `~/.cursor/auth.json`).

```sh
taskfmt-host run --task TASK-001 --repo <repository-url> --agent cursor-default --wait
```

The `cursor-default` profile sets `auth = "host"`. Do not combine that with
`CURSOR_API_KEY` in `env_secret`.

### Codex via Z.ai

Use the `codex-zai` profile to run Codex against Z.ai with the ChainArgos API key from 1Password:

```sh
taskfmt-host run --task TASK-001 --repo <repository-url> --agent codex-zai --wait
```

### Codex via Kimi

Use the `codex-kimi` profile to run Codex against Kimi K3 with the ChainArgos API key from 1Password:

```sh
taskfmt-host run --task TASK-001 --repo <repository-url> --agent codex-kimi --wait
```

## Development and release checks

After changing Rust code, reinstall the binaries and rebuild all affected images before dispatch:

```sh
cargo install --path crates/taskfmt-host --locked --bin taskfmt-host
cargo install --path crates/taskfmt --locked --bin taskfmt
cargo install --path crates/taskfmt-runtime --locked --bin taskfmt-runtime
taskfmt-host build-images --agent all --no-cache --auto
```

Run the Rust checks from repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run --bin taskfmt-host -- selftest
```

The Docker integration gate is opt-in:

```sh
taskfmt-host build-images --agent all --auto
sh harness/tests/run_docker_itest.sh
```

It reports `SKIP` when Docker or the base image is unavailable; `SKIP` is never release success.
The script requires fresh proof after both Docker bodies pass and fails on stale or incomplete
evidence.

## Repository layout

```text
crates/
  taskfmt-core/        shared library (lint, verify, progress, config)
  taskfmt/             in-container validation binary
  taskfmt-runtime/     in-container boot binary
  taskfmt-host/        host operator binary
harness/
  tests/               integration and behavior tests
  images/              taskfmt, base, Claude, Codex, and Cursor image definitions
  testdata/            bundled lint and gate corpus; do not edit as documentation
  src/                 test re-exports and embedded task prompt
  Cargo.toml           harness integration-test crate
```
