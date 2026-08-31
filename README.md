# task-format

`task-format` is a research harness for answering one practical question: how should a bounded task package be written so a coding agent produces predictable, verifiable work?

## Why this exists

Coding agents can drift when a task has unclear scope, unresolved decisions, weak completion criteria, or too much unrelated context. An agent saying that work is done is not proof that it is correct. This project tests whether a precise task contract and an independent machine gate make outcomes more reliable.

The project changes the task package, not the task itself. Each run gives one agent one bounded task in a fresh container and a fresh clone, then measures the result with the same gate.

## What a task package contains

- `README.md` defines the task-specific goal, requirements, acceptance criteria, fixed decisions, scope, and checklist.
- `AGENTS.md` defines the shared execution protocol and progress rules.
- `verify.toml` declares the completion gate: commands, required and forbidden paths, patterns, and the scope whitelist.
- `decisions.md`, when present, holds binding decisions.
- `trusted/`, when present, contains planner-owned verification material overlaid into the working repository before the agent starts.

The package is mounted read-only. The agent changes code in `/work` and keeps progress only in `/progress/progress.md`; that progress file is never committed.

## How a run works

1. `taskfmt` starts from a fresh clone or fetch of the experiment repository.
2. It overlays trusted verification material and records a trusted base commit.
3. It gives the read-only task package to one agent in a fresh headed container.
4. The agent iterates against `taskfmt verify`.
5. The host reruns the gate. Only a PASS can be promoted to `main`; failed or blocked runs are never pushed.

Predictability is computed by the harness, not self-reported by the agent. Measurements include gate pass rate, false-DONE rate, scope violations, verification evidence, and diff stability across repeated runs.

## Current status

The `task/v4` control format and Rust harness are built. `TASK-001` through `TASK-007` exercise the system by building a terminal PostgreSQL client (`pgtui`) from an empty experiment repository. They are control and shakeout work, not measured comparisons: no format-comparison results exist yet.

## Start here

- Research question, method, evidence, and decisions: [research record](docs/research/).
- Running and operating the Rust CLI: [harness guide](harness/README.md).
- Authoring a package: [task template](reference/task-template/).
- Experiment packages, fixtures, and run data: [experiments](experiments/).

## Repository map

| Path | Purpose |
| --- | --- |
| `harness/` | Rust `taskfmt` CLI: linting, verification, container dispatch, status, promotion, and experiment orchestration. |
| `reference/task-template/` | Canonical agent-visible `task/v4` package template. |
| `experiments/tasks/` | Versioned task packages used as experiment data. |
| `experiments/fixtures/` | Seed data and trusted-material specification. |
| `experiments/runs/` | Local run artifacts; ignored by Git. |
| `docs/research/` | Research question, method, evidence, findings, and decisions. |
| `experiment.toml` | Versioned experiment manifest: paths, images, runtime, and agent profiles. |

## Boundaries that protect the experiment

- The task package is the research object. Preserve `reference/task-template/` and `experiments/tasks/` structure and task metadata unless deliberately changing an experiment variant.
- A task may modify only paths listed in its `expected_paths` and `verify.toml` `allowed_globs`; the two lists must match.
- `taskfmt verify` is the completion authority. A passing agent report without a passing host gate is not a successful run.
- Runs are isolated. Do not reuse a previous workspace or container as a new experimental result.

## License

Apache License 2.0. See [LICENSE](LICENSE).
