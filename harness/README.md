# Harness

`taskfmt` runs one bounded coding task in the caller-provided execution boundary. It gates a frozen candidate tree and can promote that exact tree.

AI agents can change a repository quickly. A run record is promotion evidence only after a passing host gate has recorded an immutable candidate tree, its expected parent, and verifier evidence.

The Rust crate and `taskfmt` binary live in this directory. Repository-level settings live in [`experiment.toml`](../experiment.toml).

## Lifecycle

For each task, `taskfmt`:

1. Lints the task package before dispatch.
2. Creates a fresh workspace from the experiment repository's `main` branch.
3. Adds planner-owned trusted verification material to that workspace.
4. Starts one headed agent container with a writable `/work` workspace and read-only task instructions.
5. Records run state under `experiments/runs/`.
6. Stops the executor, stages one complete candidate tree, and runs the host gate against that tree.
7. Records gate evidence, tree, and expected parent; `promote` creates and pushes only that recorded tree with an expected-parent lease.

The harness supports Claude and Codex agent profiles. Images are built from [`images/`](images/); profile, runtime, repository, and path settings belong in [`experiment.toml`](../experiment.toml).

## Safety model

- **Task package** — planner-owned instructions and verifier inputs. Start from the [task template](../reference/task-template/README.md); configured packages are under [`experiments/tasks/`](../experiments/tasks/).
- **Trusted material** — tests, fixtures, support code, or other verifier inputs supplied by the planner. It is added to the run workspace before the agent starts and is outside the agent's allowed output paths.
- **Run** — one task, one fresh container, one recorded workspace. Its record is `experiments/runs/<id>/manifest.json`.
- **Gate** — `taskfmt verify` runs against the frozen candidate at the caller-provided boundary. Its record includes the candidate tree, parent, verifier evidence, and verdict.
- **Promotion** — creates a commit from the recorded passing tree and parent, then pushes it with an expected-parent lease. It refuses failed, incomplete, changed, or stale-parent records.

Task-package Markdown, the launch prompt, and bundled task fixtures are executable inputs. Do not casually rewrite them while changing operator documentation.

## Install and first-time setup

Run from the repository root. Docker must be running.

```sh
cargo install --path harness --locked
taskfmt preload --auto
taskfmt build-images --agent all --auto
```

`build-images --agent all` builds, in order:

1. `harness-taskfmt:latest` — the Rust `taskfmt` binary;
2. `harness-base:latest` — Debian Trixie, Node.js 24 LTS, Rust 1.98, Herdr 0.8.2, PostgreSQL client 18, and runtime tools;
3. `harness-claude:latest` — Claude Code 2.1.261;
4. `harness-codex:latest` — Codex CLI 0.153.4.

Use `--agent claude` or `--agent codex` to rebuild one agent layer. Use `--no-cache` when
refreshing moving base layers. `preload` uses the digest in
`images/preload/postgres.digest` and saves the matching PostgreSQL 18 image for the inner Docker
daemon.

Image building and runtime selection are separate. Build both images once, then choose a profile
per run.

### Claude with GLM-5.3-Flash

The `zai-flash` profile uses Claude Code through Z.ai's Anthropic-compatible API. Store only the
Z.ai token in the referenced file:

```sh
mkdir -p ~/.config/taskfmt
chmod 700 ~/.config/taskfmt
$EDITOR ~/.config/taskfmt/zai-flash.token
chmod 600 ~/.config/taskfmt/zai-flash.token
```

Run with `--agent zai-flash`, or omit `--agent` because it is the configured default:

```sh
taskfmt run --task TASK-001 --repo <repository-url> --agent zai-flash --wait
```

### Codex

Use the `codex-default` profile and the image built by `--agent all`:

```sh
taskfmt run --task TASK-001 --repo <repository-url> --agent codex-default --wait
```

## Run one task

Lint first, then dispatch to a fresh container:

```sh
taskfmt lint TASK-001
taskfmt run --task TASK-001 --repo <repository-url> --agent codex-default --wait
```

`--wait` waits for the agent, runs the host gate, and returns success only for a passing goal.
Promotion remains explicit:

```sh
taskfmt promote <run-id> --auto
```

To return immediately, omit `--wait`, then run:

```sh
taskfmt status <run-id> --wait
taskfmt gate <run-id> --auto
taskfmt promote <run-id> --auto
```

Without `--repo`, `run` creates a disposable private repository after confirmation. `--exp <id>`
records the run inside an experiment state file.

## Run experiments

`experiment` performs repository setup, dispatch, host gating, and promotion for each selected task
in order. It stops on the first failed or blocked task.

Run a range against an existing repository:

```sh
taskfmt experiment \
  --tasks 1-3 \
  --repo <repository-url> \
  --agent zai-flash \
  --auto
```

Run every configured task in one command:

```sh
taskfmt experiment \
  --tasks all \
  --repo <repository-url> \
  --agent zai-flash \
  --auto
```

Omit `--repo` to create a disposable private repository. Valid selections include `all`, `1-3,5`,
`TASK-002..TASK-004`, and `TASK-101`. Resume an interrupted experiment with its recorded ID:

```sh
taskfmt experiment --resume <experiment-id> --tasks all --agent zai-flash --auto
```

Use `--selfcheck` to run the D13 gate selfcheck before each dispatch. It refuses to dispatch when
the selfcheck fails or has no verdict.

Mutating commands prompt for confirmation. `--auto` and `--yes` skip prompts; a non-interactive
shell requires one of them. Read-only commands do not prompt.

## Follow and inspect runs

```sh
taskfmt ps
taskfmt ps --json
taskfmt status <run-id>
taskfmt status <run-id> --wait
taskfmt attach <run-id>
```

`<run-id>` may be the run ID, `harness-<run-id>` container name, run directory, or its
`manifest.json`. `attach` reconnects to the live agent TUI; detach with `ctrl+b q`, never `ctrl+c`.
Run records and evidence live under `experiments/runs/`.

## Task validation and gates

Validate task packages before dispatch:

```sh
taskfmt lint TASK-001
taskfmt lint --json
taskfmt progress-init TASK-001
```

Run a gate directly in a workspace with `taskfmt verify`:

```sh
taskfmt verify \
  --root <repository-root> \
  --task-dir <task-directory> \
  --progress <progress-file>
```

The gate passes only when the declared checks pass and the final stdout line is `DONE`. It also
checks expected results, writable paths, scope, and progress unless progress is disabled with
`--no-progress` or `--progress ""`. Use `--fail-fast` to stop after the first failed check.

Prove a task gate distinguishes the untouched base from a reference solution:

```sh
taskfmt selfcheck <task-directory> <base-workspace> \
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

## Inspection and repository lifecycle

Compare the host binary fingerprint with an image:

```sh
taskfmt fingerprint
taskfmt fingerprint --image harness-claude:latest
taskfmt fingerprint --image harness-codex:latest
taskfmt fingerprint --path harness
```

Manage disposable GitHub repositories explicitly when needed:

```sh
taskfmt repo create --auto
taskfmt repo delete --name <repository-name> --auto
```

`selfhost` is an advanced, separate command family. Its complete command reference is available
from `taskfmt selfhost --help` and its subcommands' help.

## Development and release checks

After changing Rust code, the host binary and image-baked binary must match. Reinstall and rebuild
all affected images before dispatch:

```sh
cargo install --path harness --locked
taskfmt build-images --agent all --no-cache --auto
```

Run the Rust checks from repository root:

```sh
cargo fmt --manifest-path harness/Cargo.toml --check
cargo clippy --manifest-path harness/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path harness/Cargo.toml
cargo run --manifest-path harness/Cargo.toml -- selftest
```

The Docker integration gate is opt-in:

```sh
taskfmt build-images --agent all --auto
sh harness/tests/run_docker_itest.sh
```

It reports `SKIP` when Docker or the base image is unavailable; `SKIP` is never release success.
The script requires fresh proof after both Docker bodies pass and fails on stale or incomplete
evidence.

## Repository layout

```text
harness/
  src/                 taskfmt implementation
  tests/               integration and behavior tests
  checks/              fingerprint command checks
  images/              taskfmt, base, Claude, and Codex image definitions
  testdata/            bundled lint and gate corpus; do not edit as documentation
  goal-prompt.md       dispatched runtime input; do not edit as documentation
  Cargo.toml           crate manifest
```
