# Experiments

This directory contains the versioned inputs and generated records for the task-format study. The runner lives in [`../harness/`](../harness/README.md); this directory is the experiment data boundary.

The executable harness lives in [`../harness/`](../harness/README.md). This directory contains no execution scripts.

## Contents

| Path | Purpose | Lifecycle |
| --- | --- | --- |
| [`tasks/`](tasks/) | The `TASK-001`–`TASK-007` packages for the `pgtui` example application. | Authored experiment input. |
| [`fixtures/seed/`](fixtures/seed/) | Default PostgreSQL seed SQL copied into every run. | Shared static input. |
| `runs/` | Workspaces, task snapshots, progress files, logs, and manifests created by dispatch. | Generated and gitignored. |

## Package contract

Each package contains the task contract (`README.md`), fixed decisions (`decisions.md`), gate configuration (`verify.toml`), and planner-owned verification material (`trusted/`). The task contract, decisions, and gate configuration are mounted read-only for the agent. The harness adds the standard task protocol from [`reference/task-template/`](../reference/task-template/) when a package does not carry it.

Before an agent starts, the harness creates a fresh clone of the experiment repository's `main` branch, overlays that task's `trusted/` material, and commits the overlay as the run's base commit. The agent works only in that clone. `expected_paths` and `allowed_globs` restrict what it may change, so trusted tests and support files remain outside its scope. A passing run can be promoted to `main`; a failed run is never promoted.

The series starts from an empty repository. TASK-001 creates the Rust workspace, and later tasks build `pgtui` one gated slice at a time. No task relies on fixture-supplied application code.

## Operator entry points

From the repository root:

```sh
taskfmt lint                 # validate every task package
taskfmt run --task TASK-001  # dispatch one fresh run
taskfmt experiment --tasks all --repo <URL>
```

`experiment.toml` defines `tasks_dir`, `runs_dir`, and `seed_dir`. For dispatch, gates, promotion, run records, and safety rules, see [`harness/README.md`](../harness/README.md).

## Boundaries

- Do not put harness code or run-specific state here.
- Do not edit `runs/`; it is evidence produced by a run.
- Treat a task package with a completed run as immutable. Carry discoveries into a new documented change rather than changing history beneath an existing result.
- Keep planner-owned material in the task's `trusted/` directory, not in a shared application fixture.
