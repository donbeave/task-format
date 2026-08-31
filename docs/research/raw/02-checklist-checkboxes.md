# Archive — raw research 02: checklist progress

- **Source date:** 2026-08-28.
- **Purpose:** Explore how an agent can track a bounded task across turns without treating a completion claim as evidence.
- **Durable findings:** A task needs a fixed, hierarchical checklist; work is tracked at leaf items; each completed leaf needs direct evidence; and resumed work needs an explicit current item and concise handoff state. Missing or contradictory work requires replanning, not a silent checklist rewrite.
- **Adopted / superseded:** `task/v4` adopts numbered, evidence-bearing checklist leaves and machine-checked progress. The proposed design that made the task contract writable and used it as the sole progress store is superseded: the task package is now read-only, while `progress.md` is generated as derived execution state and checked against the package.
- **Current authority:** [task template](../../../reference/task-template/), [execution protocol](../../../reference/task-template/AGENTS.md), [harness guide](../../../harness/README.md), and [research findings](../RESEARCH-FINDINGS.md).
