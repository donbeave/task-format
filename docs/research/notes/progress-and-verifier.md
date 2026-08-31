# Archive — progress and verifier design

- **Source date:** 2026-08-28.
- **Purpose:** An early design exploration for durable agent progress records and a trusted completion verifier.
- **Status:** Historical. The described Bash files and trust model are not operating instructions.

## Durable findings

- Progress records help a fresh agent resume work, but never prove completion.
- A completion claim is valid only when an independent machine gate passes.
- Scope must be checked against a recorded baseline, not an agent's description of its changes.
- The task contract and planner-owned verification inputs must remain outside the executor's editable scope.
- Preserve logs and artifacts so a verdict can be inspected after a run.

## Adopted and superseded

The project adopted external gating, fixed-baseline scope checks, read-only task packages, and a non-committed progress record.

The proposed `.task/` layout, `verify.sh`, environment-variable trust switching, checksum manifest, and checkbox serialization are superseded by the task/v4 package, `verify.toml`, trusted overlays, and the Rust `taskfmt` harness.

## Current authority

- [Research findings](../RESEARCH-FINDINGS.md) — current decisions and rationale.
- [Core concepts](../../concepts.md) — current terms for trusted material, scope, gates, and progress.
- [Task-package reference](../../../reference/task-template/) — current package rules.
- [Harness guide](../../../harness/README.md) — current verification and run behavior.
