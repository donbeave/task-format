# Task progress monitoring

The monitor adds a filesystem catalog and browser to the existing taskfmt harness.
It does not edit task contracts, create tasks, or replace taskfmt's verification.

## Layout and identifiers

```text
projects/
  jackin/
    README.md
    new-design/
      README.md
      001/
        README.md
        AGENTS.md
        verify.toml
        task.toml
        trusted/
```

Project and group README files must contain a nonempty H1 display name.
Their remaining Markdown supplies the description.
Directory codes are normalized to lowercase for catalog identifiers.
Codes contain ASCII letters, digits, and internal hyphens; task numbers contain
3–12 digits, conventionally `001`, `002`, and so on.
`jackin/new-design/001` is a catalog identity, independent of the immutable
`TASK-001` contract ID inside the README.
Case-normalized collisions invalidate the catalog.

Existing `experiments/tasks/TASK-*` packages and CLI selection syntax remain
supported. To migrate a package, preserve its README, verification configuration,
execution instructions, and trusted assets; place it beneath a project/group
directory and add the metadata sidecar. Never rewrite strict README frontmatter
to hold browser state or hierarchical IDs.

## Metadata schema

```toml
schema = "task-meta/v1"
status = "pending"
dependencies = ["jackin/new-design/001"]
```

The schema has exactly these three fields; unknown fields are rejected.
Valid statuses are `draft`, `pending`, `in_progress`, and `done`.
Dependencies must be canonical fully qualified IDs, including for tasks in the
same group. Missing references, duplicate references, self-references, invalid
paths, and dependency cycles invalidate the whole catalog visibly.
Cross-project dependencies are permitted and obey the same readiness rules.
A group/project run never expands into another scope: unfinished external
dependencies prevent that scope from starting.

Draft tasks appear in the catalog but never execute and do not block readiness.
Pending tasks are ready when all non-draft dependencies are done.
Blocked is the computed subset of pending tasks with unfinished dependencies;
therefore pending and blocked counts overlap.
In-progress tasks belong to an execution; clients cannot set this state.
Done requires successful authoritative taskfmt verification, not an agent's
completion claim or an HTTP status update.
There is no browser status mutation endpoint.

Metadata changes are atomic. The monitor holds operating-system locks for the
catalog and run roots, binds run data durably to its catalog, reserves execution scopes, and
uses bounded scheduling across ready DAG branches.
Failure preserves run evidence and returns the failed task to pending after
cleanup is confirmed. Unconfirmed cleanup keeps `in_progress` and records
`cleanup_required`; resolve the logged cause and restart for reconciliation.
Dependents of a failed prerequisite do not start.
On restart the monitor reconciles durable execution evidence before serving.
Shutdown cancels owned child processes and stops their persistent containers.
An unrecoverable scheduling error halts execution with a visible disabled reason
until the operator resolves the cause and restarts the backend.

## Progress and authority

Task README and verify.toml define the immutable contract.
The sidecar records lifecycle status; successful host gate evidence authorizes
done. Durable monitor records associate canonical catalog IDs with taskfmt runs.
Taskfmt's manifest, frozen candidate, and host gate remain verification authority.
`progress/v1` is coordination evidence only.

Task percentage is 100 for done; otherwise it is completed checklist leaves
divided by total checklist leaves, multiplied by 100. Missing progress is zero.
The UI also exposes the current leaf, latest event, failure, execution state,
and latest run result when available.
Project/group percentage is the arithmetic mean of their task percentages,
including drafts. Counts and percentages are computed, never persisted.

## Browser and API

Routes are `/`, `/projects/$projectCode`,
`/projects/$projectCode/groups/$groupCode`,
`/projects/$projectCode/groups/$groupCode/tasks/$taskNumber`, and `/tasks/my`.
My Tasks shows topological dependency lanes, parallel-ready tasks, prerequisites,
and downstream tasks that may become runnable.
Kanban uses four persisted-status columns with blocked pending cards identified.

The API exposes `GET /api/catalog`, `GET /api/executions`, and:

```text
POST /api/run/task/{project}/{group}/{number}
POST /api/run/group/{project}/{group}
POST /api/run/project/{project}
```

Run actions require `X-Task-Monitor: 1` and an allowed browser origin.
The browser sends validated identifiers only. Repository, executable, profile,
configuration, projects root, and run root are operator settings.
Markdown rendering does not allow unsafe HTML.
Polling refreshes progress and eligibility without WebSockets.
Dependency readiness is distinct from execution eligibility: a ready task can
still have a disabled action when execution is unconfigured or Docker is unavailable.

## Local development

Use Rust 1.98.1 and Bun 1.4.2. `rust-toolchain.toml` pins Rust; an existing
`RUSTUP_TOOLCHAIN` environment override takes precedence, so the commands below
select the version explicitly. Run from the repository root:

```sh
cargo +1.98.1 build --manifest-path harness/Cargo.toml --bins
cd web
bun install --frozen-lockfile
bun run dev
```

In a second terminal, from the repository root:

```sh
harness/target/debug/task-monitor --origin http://127.0.0.1:5173
```

Open `http://127.0.0.1:5173`. The frontend proxies `/api` to
`http://127.0.0.1:3001`; the server listens only on loopback.
Defaults are `--projects-root projects`, `--runs-root .monitor-runs`, and
`--concurrency 2`. Concurrency accepts 1–32.
Keep generated run state and lock files outside version control.

## Catalog-root migration and rollback

The primary catalog directory is now `projects/`; legacy experiment packages
remain under `experiments/tasks/`, and browser `/tasks/my` and task-detail URLs
are unchanged. A custom catalog is selected with
`--projects-root /absolute/path/to/projects`.

Run roots store a durable `catalog-root` binding to the canonical catalog path.
Renaming `tasks/` to `projects/` does not migrate that binding or the run evidence.
Never delete `.monitor-runs`, edit its binding manually, or reset verified
metadata just to make a relocated catalog start.

1. Stop the monitor gracefully and confirm its executors have stopped. Preserve
   the catalog and its entire run root together, including metadata and evidence.
2. If the old catalog has execution evidence, continue using its original path
   with `--projects-root /absolute/path/to/tasks --runs-root /absolute/path/to/.monitor-runs`.
   A binding-aware relocation procedure is not provided. Keep this pair together
   until such a migration is available.
3. For an untouched pending/draft example catalog, move it to `projects/` and
   select a new, separate run root if `.monitor-runs` was already bound to the
   old path. Preserve the old run root; do not repoint or erase it.
4. To roll back, stop the new monitor, restore the preserved catalog at its
   original canonical path, and restart with its matching preserved run root.
   Do not combine one catalog's metadata with another catalog's run evidence.

## Execution configuration

To enable execution, start the backend with operator-owned settings:

```sh
harness/target/debug/task-monitor \
  --origin http://127.0.0.1:5173 \
  --taskfmt harness/target/debug/taskfmt \
  --config experiment.toml \
  --repo '<existing-repository-url>' \
  --agent codex-default
```

Build all binaries together: `task-monitor-supervisor` must remain beside
`task-monitor`. Its pipe and filesystem lease couple executor lifetime to the
backend and allow restart recovery without trusting reusable process IDs.

## Project and group CLI

Project/group commands are first-class `taskfmt` subcommands. Read operations
scan the filesystem and validate the catalog; no monitor process is required:

```sh
taskfmt project list --projects-root projects --json
taskfmt project show demo --projects-root projects
taskfmt project lint demo --projects-root projects
taskfmt group list demo --projects-root projects
taskfmt group show demo/pgtui --projects-root projects
taskfmt group lint demo/pgtui --projects-root projects
```

Read operations accept `--json`. If `--projects-root` is omitted, the root comes
from `paths.projects_dir` in the resolved experiment configuration. An explicit
root does not require an experiment configuration. Reads describe persisted
filesystem metadata, not live execution progress or a new verification verdict.

Run operations submit a validated canonical scope to the configured monitor:

```sh
taskfmt project run demo --monitor-url http://127.0.0.1:3001 --wait
taskfmt group run demo/pgtui --monitor-url http://127.0.0.1:3001 --wait
```

Choose one scope; these are alternative examples. The monitor URL defaults to
`http://127.0.0.1:3001`. Its configured projects root, repository, agent, and
execution settings determine what runs; a CLI read-root override does not
reconfigure the server. The commands retain the harness consent protocol;
global `--auto` or `--yes` may be used when explicitly choosing unattended
execution. `--json` supports machine-readable output.

Without `--wait`, the command returns after the server accepts the execution.
With `--wait`, it polls the exact returned execution ID, exits zero for `done`,
and exits nonzero for other terminal outcomes. It does not mark tasks done,
skip dependency checks, promote, or push code itself.

## Execution prerequisites

Read-only monitoring works without agent credentials or a configured execution
repository. Execution uses the installed `taskfmt`, an existing experiment
configuration, and an explicit repository chosen by the operator.
The normal taskfmt Docker images, prerequisites, and selected profile credentials
must be configured as described in `harness/README.md`.
After harness source changes, rebuild images so host/image fingerprints match.

Runs call the existing wait-and-gate lifecycle. The monitor does not promote or
push candidate code. Original sequential task contracts retain their predecessor
provenance checks: a verified dependency alone does not prepare the repository
baseline for the next contract. Use the existing operator promotion workflow when
those contracts require it. See `monitoring-examples.md` for the seeded corpus.

See `monitoring-versions.md` for exact toolchain and dependency versions and
`monitoring-architecture.md` for invariants and implementation boundaries.

## Production build and verification

Build Rust executables together and the frontend through Bun:

```sh
cargo +1.98.1 build --manifest-path harness/Cargo.toml --release --bins
cd web
bun run build
bun run preview
```

Stop the development frontend before preview; both use port 5173 and proxy the
API. Start `harness/target/release/task-monitor` in the backend terminal.
The generated Start SPA is `web/dist/client/_shell.html`; a separate static
server must provide route fallback to that file and proxy `/api` to the backend.

Run all gates from the repository root:

```sh
cargo +1.98.1 build --manifest-path harness/Cargo.toml --bins
cargo +1.98.1 fmt --manifest-path harness/Cargo.toml -- --check
cargo +1.98.1 clippy --manifest-path harness/Cargo.toml --all-targets -- -D warnings
cargo +1.98.1 test --manifest-path harness/Cargo.toml
cargo +1.98.1 run --manifest-path harness/Cargo.toml --bin taskfmt -- selftest
RUSTUP_TOOLCHAIN=1.98.1 sh harness/tests/run_docker_itest.sh
cd web
bun install --frozen-lockfile
bun run lint
bun run typecheck
bun run test
bun run build
```

The Docker proof script requires a live Docker daemon and the harness base image.
Ordinary `cargo test` explicitly skips that suite when not enabled.
Monitor integration tests use the production HTTP scheduler, process adapter,
supervisor, and actual immutable host gate. A deterministic executable substitutes
for taskfmt dispatch and agent work, preparing local gate inputs before invoking
the real gate. Separate CLI, lifecycle and Docker suites cover their boundaries.
The monitor tests verify success, failure, conflicting starts,
dependency ordering, parallel branches, and the concurrency bound.
They do not claim a paid agent solved the seeded pgtui contracts.
