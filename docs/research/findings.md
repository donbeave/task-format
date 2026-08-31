# Findings

## Current answer

Task-package structure is a credible engineering variable. Explicit scope,
fixed decisions, machine-checkable acceptance, and fresh run state target real
failure modes in autonomous coding: drift, hidden scope growth, stale context,
and unsupported claims of completion.

The project has not yet proved that one writing style wins. The current result
is a testable hypothesis plus a working lifecycle, not a universal recipe.

## What is ready

- `task/v4` is the control package format.
- `taskfmt` can lint, dispatch, verify, record, and promote runs.
- `TASK-001` through `TASK-007` exercise the lifecycle by building `pgtui` in
  ordered slices from an empty experiment repository.

These packages are control and shakeout inputs. They are not a completed
comparison of alternative task-writing styles.

## What remains unproven

No controlled ablation matrix currently shows a causal improvement from a
specific wording, checklist, or rule-placement change. The next useful result
must compare one such change with the control over repeated fresh runs.

## Interpretation rule

Treat a passing run as evidence about that run. Treat an improvement as a
research finding only when the controls held, the host gate decided the result,
and repeated records show the effect and its variation.

See [method](method.md) for controls and [evidence](evidence.md) for the proof
standard.
