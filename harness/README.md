# Harness

`taskfmt` runs one bounded coding task in an isolated container. Host gating and promotion are temporarily disabled until verification uses an isolated immutable candidate.

AI agents can change a repository quickly. Task dispatch and in-container verification remain available. Do not treat legacy run records as secure promotion evidence.

The Rust crate and `taskfmt` binary live in this directory. Repository-level settings live in [`experiment.toml`](../experiment.toml).

## Lifecycle

For each task, `taskfmt`:

1. Lints the task package before dispatch.
2. Creates a fresh workspace from the experiment repository's `main` branch.
3. Adds planner-owned trusted verification material to that workspace.
4. Starts one headed agent container with a writable `/work` workspace and read-only task instructions.
5. Records run state under `experiments/runs/`.
6. Stops before host gating or promotion. Those operations require the pending secure `run/v1` boundary.

The harness supports Claude and Codex agent profiles. Images are built from [`images/`](images/); profile, runtime, repository, and path settings belong in [`experiment.toml`](../experiment.toml).

## Safety model

- **Task package** — planner-owned instructions and verifier inputs. Start from the [task template](../reference/task-template/README.md); configured packages are under [`experiments/tasks/`](../experiments/tasks/).
- **Trusted material** — tests, fixtures, support code, or other verifier inputs supplied by the planner. It is added to the run workspace before the agent starts and is outside the agent's allowed output paths.
- **Run** — one task, one fresh container, one recorded workspace. Its record is `experiments/runs/<id>/manifest.json`.
- **Gate** — `taskfmt verify` runs inside the task container. Legacy host gate records are inspectable but cannot be created or promoted securely.
- **Promotion** — disabled until a record binds an immutable tree, trusted Git metadata, isolated verifier evidence, and expected remote parent.

Task-package Markdown, the launch prompt, and bundled task fixtures are executable inputs. Do not casually rewrite them while changing operator documentation.

## Common commands

Preload the pinned prerequisite image, then build agent images when the local image cache is not ready:

```sh
taskfmt preload
taskfmt build-images --agent all
```

Run one task against an existing repository:

```sh
taskfmt lint TASK-001
taskfmt run --task TASK-001 --repo <repository-url> --agent codex-default
taskfmt status <run-id> --wait
# Host gate and promotion are disabled pending secure run/v1 records.
```

Without `--repo`, `taskfmt run` can create a disposable private experiment repository. To run a selected series, use the complete loop:

```sh
taskfmt experiment --tasks 1-3 --repo <repository-url> --agent codex-default
```

Commands that change state or spend substantial resources ask for confirmation. Use `--auto` or `--yes` for unattended execution; a non-interactive shell requires one of them. Read-only commands do not prompt.

## Follow a run

```sh
taskfmt ps
taskfmt status <run-id>
taskfmt attach <run-id>
# Host gate is disabled pending isolated immutable verification.
```

`attach` reconnects to the agent TUI; detach with `ctrl+b q`, not `ctrl+c`. A `<run-id>` may also be the container name, run directory, or that run's `manifest.json` path. `taskfmt ps` works without a manifest and is the quickest way to find local runs.

## Verification and safety

`taskfmt lint` checks task-package structure before a run. `taskfmt progress-init` creates the agent's uncommitted progress file from the package README.

`taskfmt verify` runs commands declared by the package's `verify.toml`, checks the allowed change scope, and checks progress unless disabled explicitly. `taskfmt gate` and `taskfmt promote` fail closed because legacy runs expose executor-controlled code and Git metadata to host processes.

Use `--selfcheck` with `run` or `experiment` to test a task gate before dispatch. `taskfmt selfcheck` requires the untouched base to fail relevant focused checks; with a reference solution, it also requires the gate to pass. Selfcheck is opt-in because it uses the task's host toolchain; dispatch refuses both a failing result and a no-verdict result.

Secrets in agent profiles are references, not values. They are resolved only at dispatch, passed through a temporary mode-0600 environment file, then redacted from harness output and records.

## Configuration

Every command that needs repository layout, images, or profiles reads [`experiment.toml`](../experiment.toml). Selection order is:

1. `--config <path>`
2. `TASKFMT_CONFIG`
3. Nearest `experiment.toml` found by walking upward from the current directory

Paths in the manifest resolve relative to that manifest, so commands can run from any subdirectory. Set agent profile, model, effort, static environment, secret references, image names, task directory, run directory, and runtime limits there.

## Useful commands

```text
taskfmt lint [TASKS...]              validate task packages
taskfmt progress-init <TASK>         create initial progress file
taskfmt verify [FLAGS]               run task completion gate
taskfmt selfcheck <TASK> <WORKSPACE> prove gate distinguishes base from solution
taskfmt run --task <TASK>             dispatch one containerized task
taskfmt status <RUN> [--wait]         inspect or wait for a run
taskfmt attach <RUN>                  reconnect to agent TUI
taskfmt gate <RUN>                    disabled pending secure run/v1 records
taskfmt promote <RUN>                 disabled pending secure run/v1 records
taskfmt experiment [FLAGS]            run selected tasks in order
taskfmt ps [--json]                  list local run containers
taskfmt build-images [--agent ...]   build harness images
taskfmt preload                       cache pinned prerequisite image
taskfmt fingerprint [FLAGS]          inspect host, source, or image binary fingerprint
taskfmt selftest                      test bundled harness corpus and gate behavior
```

Run `taskfmt --help` or `taskfmt <command> --help` for command flags. `selfhost` is an advanced, separate command family; its subcommand help is the operator reference.

## After changing this crate

The host binary and image-baked binary must match. `taskfmt run` compares their content fingerprints and refuses dispatch when they differ. After changing harness Rust code, reinstall the binary and rebuild the affected images.

Run the harness checks from repository root:

```sh
cargo fmt --manifest-path harness/Cargo.toml --check
cargo clippy --manifest-path harness/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path harness/Cargo.toml
cargo run --manifest-path harness/Cargo.toml -- selftest
```

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
