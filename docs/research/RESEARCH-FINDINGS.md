# Research findings

## Conclusion

Task-package structure is a testable part of an autonomous coding system. It can influence whether an agent understands its boundary, reaches a working implementation, proves completion, and avoids unrelated changes.

The project tests that effect with controlled runs. It does not treat a persuasive final report as evidence.

## Research question

For one bounded software outcome, which task-package structure produces the most predictable agent result when the environment and verification remain comparable?

## Method

The experiment changes the written package, not the surrounding system:

- One bounded outcome per run.
- Fresh agent, fresh container, and fresh repository clone.
- Recorded baseline before the agent begins.
- Trusted verification material prepared outside the agent’s editable scope.
- Independent host-side gate after execution.
- Promotion to main only after a passing gate.

This isolates task writing from previous workspace state, mutable tests, and agent self-reporting.

## Current decisions

### Measure machine evidence

The gate, not the agent, decides whether a run passed. Record pass rate, completion claims that conflict with the gate, scope violations, and diff stability across repeats.

### Keep inputs immutable during execution

The agent receives a read-only task package. Planner-owned verification support is separate from the editable worktree, which prevents a run from weakening its own proof of completion.

### Scope changes against a fixed baseline

The harness records the run baseline and checks the result against it. An agent may change only the paths authorized for that task.

### Preserve observability

Runs use persistent containers and retain logs, snapshots, commits, and gate records. A terminal result can therefore be inspected instead of inferred.

### Compare one variable at a time

A control format is useful only when variants change one writing choice at a time. Candidate improvements need repeated comparison against the control before becoming default practice.

### Separate operating truth from research rationale

Harness code and versioned configuration define current behavior. The reference task package defines current task-format rules. This document explains why those choices exist and what still needs experimental proof.

## Current state

The repository contains:

- The task/v4 control format.
- The taskfmt Rust harness.
- Seven ordered pgtui task packages that exercise dispatch, gating, and promotion.

These inputs validate the lifecycle. They are not yet evidence that one task-package variant outperforms another; the controlled ablation matrix remains future work.

## Open evidence

The next useful evidence is repeated, one-variable-at-a-time comparison of candidate writing changes against the control. Each comparison should show that the gate still distinguishes incomplete and complete work, retain run artifacts, and report variance rather than a single favorable run.

## Sources

Historical research and external-source notes remain in this directory for provenance. They include drafts and superseded designs. Read them through the source-priority rules in [the research index](README.md), never as a replacement for current code, configuration, or task packages.
