# Archive — initial goal-template research brief

- **Source date:** 2026-08-28.
- **Purpose:** The initial research brief that asked how long-running coding work could be divided into bounded, independently verifiable tasks.
- **Status:** Provenance only. It is not a prompt, a task template, or a source of runtime instructions.

## Durable findings

- A task should describe one bounded, observable outcome with explicit scope and a stopping condition.
- Fresh agent context works best when the contract carries the relevant decisions, dependencies, and acceptance evidence.
- Split a broad goal into coherent vertical slices when those slices can be independently gated.
- Separate an agent's implementation report from the system that decides whether the work passed.
- Treat unresolved product, compatibility, or architecture choices as planning work, not executor discretion.

## Adopted and superseded

The project adopted bounded packages, fresh isolated runs, a host-side gate, and evidence-based promotion.

The original `/goal` wording, YAML shape, `task.md`/`verify.sh` package layout, and linked external guidance are superseded. Current behavior belongs to versioned repository materials, not this historical transcript.

## Current authority

- [Research findings](../RESEARCH-FINDINGS.md) — current research conclusion and method.
- [Why structured tasks](../../why-structured-tasks.md) — concise problem statement.
- [Task-package reference](../../../reference/task-template/) — current authoring contract.
- [Harness guide](../../../harness/README.md) — current run, verification, and promotion behavior.
