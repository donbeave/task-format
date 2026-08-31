# Project guide

`task-format` tests whether a structured task package helps an AI coding agent
produce work that is predictable, bounded, and independently verifiable.

This page combines the project's purpose, design rationale, operating model,
and research record. It is the starting point for understanding the system.

## Why this matters

An agent turns prose into edits, checks, and a completion claim. Ambiguous scope,
unresolved decisions, weak acceptance criteria, and stale workspace state can
produce a confident but incorrect result. AI increases the cost of ambiguity:
an agent can modify a repository faster than a reviewer can detect the wrong
boundary.

Task-format makes the boundary explicit, starts each run from fresh state, and
checks the result with a gate the agent cannot rewrite.

## The task package

One package describes one bounded software outcome:

- `README.md` — goal, context, scope, requirements, acceptance criteria, and checklist;
- `AGENTS.md` — execution protocol and progress-file rules;
- `verify.toml` — commands, path rules, and the completion gate;
- `decisions.md` — binding choices, when needed; and
- `trusted/` — planner-owned tests or fixtures outside the agent's scope.

The package is read-only during execution. The agent edits `/work` and records
resumable progress separately.

## One run

1. `taskfmt lint` validates the package.
2. The harness creates a fresh clone and records its baseline commit.
3. Trusted verification material is overlaid and recorded.
4. One agent works in one persistent container.
5. `taskfmt verify` checks commands, progress, and allowed paths.
6. The host repeats the gate and retains the run record.
7. Only a passing result may be promoted to `main`.

The agent's report is useful context, not proof. A run record includes the task
snapshot, baseline, result, logs, container state, and gate verdict.

## Research design

The question is: for the same bounded outcome, which task-package structure
produces the most predictable result?

Compare one writing change at a time: checklist shape, rule placement, context
order, or verification presentation. Keep the outcome, starting repository,
trusted checks, gate, image, agent profile, and runtime limits fixed. A single
successful run is a signal, not a finding.

Measure gate pass rate, false-completion claims, scope violations, diff
stability, retries, and rework across repeated fresh runs. Adopt a format
change only when repeated control comparisons show improvement without
weakening proof or changing the task outcome.

## Current result

`task/v4` and the Rust `taskfmt` harness are operational. `TASK-001` through
`TASK-007` exercise the lifecycle while building the `pgtui` example in ordered
slices. They are control and shakeout inputs; no completed ablation matrix yet
proves that one writing style beats another.

## Principles

- Proof is independent: the host gate decides success.
- Inputs are protected: the agent cannot rewrite its contract or trusted proof.
- Scope has a baseline: unauthorized paths fail verification.
- Runs stay inspectable: missing artifacts mean missing evidence.
- Format changes need comparison: plausible advice is not evidence.

## Next links

- [Documentation index](README.md)
- [Experiment corpus](../experiments/README.md)
- [Harness guide](../harness/README.md)
- [Task template](../reference/task-template/README.md)
- [`research/`](research/) — preserved historical source, not current guidance.
