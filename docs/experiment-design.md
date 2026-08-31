# Experiment design

## Question

For the same bounded coding objective, which task-package structure makes agent output most predictable?

## Design rule

Change one meaningful writing variable at a time. Keep the software objective, starting repository state, verification, container image, and agent configuration fixed for the comparison being made.

Examples of candidate variables include checklist shape, rule placement, and how verification is surfaced. A variant earns adoption only after it is compared with the control format.

## Controlled run

Each run uses:

- A fresh clone of the experiment repository.
- A recorded baseline commit.
- One task package and its planner-owned verification support.
- One fresh, persistent container and one agent session.
- A host-side gate separate from the agent’s report.

The same sequence prevents hidden state, changed tests, or a reused workspace from masquerading as a documentation effect.

## Evidence

The harness records outcomes that can be compared across repeated runs:

- Gate pass or failure.
- Completion claims that disagree with the gate.
- Scope violations.
- Diff stability and amount of rework.
- Run artifacts needed to inspect an unexpected result.

A result is useful only when the experiment preserved its constants and the gate could make a real verdict.

## Current status

The repository has a task/v4 control format, a Rust harness named taskfmt, and seven ordered pgtui task packages that exercise the lifecycle end to end. They are shakeout inputs for the harness and package design, not yet a completed controlled comparison matrix.

The next research phase is repeated, one-variable-at-a-time ablations against that control.
