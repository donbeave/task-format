# task-format

`task-format` is a research harness for one practical question:

> Can a structured task package make AI coding work more predictable, bounded, and independently verifiable?

## Why this exists

An AI coding agent turns prose into edits, checks, and a completion claim. If scope, decisions, or proof are unclear, it can drift into unrelated work or report success without solving the problem. Faster agents make this ambiguity more expensive, not less.

This project treats task writing as an engineering variable. It gives an agent one explicit contract, isolates the run, and checks the result outside the agent's control.

## What this project is—and is not

It is a versioned task format, a Rust harness, and an experiment corpus for testing task-package design.

It is not a model leaderboard, a universal prompting recipe, or proof that the current format is optimal. The current `TASK-001`–`TASK-007` series validates the harness and package lifecycle; it is not yet a measured format comparison.

## The task package

One package describes one bounded, observable outcome:

- `README.md` — goal, context, requirements, scope, acceptance criteria, and checklist;
- `AGENTS.md` — execution protocol and progress rules;
- `verify.toml` — commands, path limits, and completion gate;
- `decisions.md` — binding choices when the executor must not improvise; and
- `trusted/` — planner-owned tests or fixtures outside the agent's edit scope.

The package is read-only during a run. Progress is derived state stored outside the package and is never itself proof of completion.

## Trust model

The harness protects the experiment in layers:

1. A fresh clone, container, and agent session prevent previous state from contaminating a run.
2. A recorded baseline anchors scope checks to a known commit.
3. Trusted verification material is overlaid before execution and kept outside the agent's allowed paths.
4. `taskfmt verify` checks the task's declared commands, progress, and scope.
5. The host repeats the gate after the agent stops. The host verdict, not the agent's report, decides success.
6. Host gating and promotion remain disabled until secure `run/v1` records bind isolated verification to an immutable tree.

## Operating a run

Use this sequence:

1. `taskfmt lint TASK-001`
2. `taskfmt selfcheck TASK-001 <workspace>`
3. `taskfmt run --task TASK-001 --repo <repository-url> --agent codex-default`
4. `taskfmt status <run-id> --wait` or `taskfmt attach <run-id>`
5. Inspect the run record. Host `gate` and `promote` currently fail closed pending the secure boundary.

Use `taskfmt experiment --tasks 1-3 --repo <repository-url>` for an ordered series. See [harness/README.md](harness/README.md) for setup, flags, safety, configuration, and development checks.

## Research question and method

The research compares task-package writing, not software outcomes. Change one meaningful variable—such as checklist shape, rule placement, context order, or verification presentation—while holding the outcome, repository, trusted checks, gate, image, agent profile, and runtime limits fixed.

Each observation is a fresh run with a recorded baseline, an independent gate, and retained artifacts. A single favorable run is a signal, not a finding.

Measure gate pass rate, false-completion claims, scope violations, diff stability, retries, and rework across repeated runs. Adopt a format change only when control comparisons show a reproducible improvement without weakening verification or changing the task outcome.

## Topics and adopted decisions

| Research topic | Current decision |
| --- | --- |
| Broad work vs. bounded work | Split into coherent, independently verifiable vertical slices. |
| Executor invention vs. prepared design | Resolve consequential product, architecture, and compatibility choices before dispatch. |
| Mutable contract vs. protected contract | Keep task instructions and planner-owned proof read-only. |
| Self-report vs. independent proof | Let the host-side gate decide completion. |
| Shared mutable checks vs. trusted checks | Overlay verifier inputs outside the agent's allowed paths. |
| Reused state vs. fresh state | Start each observation from a fresh clone, container, and session. |
| Pixel-only UI proof vs. portable evidence | Prefer deterministic semantic or textual assertions for interactive behavior. |
| Repeated prose vs. enforceable rules | Put critical invariants in the harness and configuration. |

These are design decisions and hypotheses, not claims that alternatives always fail.

## Findings and open evidence

Research converged on five durable principles: bounded outcomes, settled decisions, protected inputs, independent gates, and inspectable fresh runs. `task/v5`, `verify/v2`, `taskfmt`, and seven ordered `pgtui` packages implement those ideas.

No completed ablation matrix yet proves that one wording or checklist style produces a quantitative improvement. Future claims require repeated control comparisons and their full run records.

## Repository map

| Path | Role |
| --- | --- |
| `harness/` | Rust `taskfmt` CLI and operator reference. |
| `experiments/tasks/` | Versioned task packages used as experiment inputs. |
| `experiments/fixtures/` | Shared deterministic seed data. |
| `experiments/runs/` | Generated run workspaces and evidence; Git-ignored. |
| `reference/task-template/` | Canonical `task/v5` + `verify/v2` package template. |
| `experiment.toml` | Versioned paths, images, runtime, and agent profiles. |

## Authority and boundaries

When documents disagree, use this order: harness code and `experiment.toml`, then the task template and live experiment packages, then this documentation. Documentation explains behavior; it does not override executable rules.

Do not alter task-package structure, task metadata, trusted verification, or fixtures while changing explanatory documentation. A task may change only its declared `expected_paths` and matching `allowed_globs`.

## License

Apache License 2.0. See [LICENSE](LICENSE).
