# Archive — task author gate v1

- **Source date:** 2026-08-28.
- **Purpose:** Propose a preflight checklist for deciding whether planning material was mature enough to become an agent-executed task package.
- **Durable findings:** A runnable task needs one independently demonstrable outcome, settled decisions, explicit boundaries, a verification seam, falsifiable acceptance evidence, and real prerequisites. A task should not conceal unresolved design choices or depend on future work for its own proof.
- **Adopted / superseded:** The task/v4 package and `taskfmt lint` enforce the parts that can be checked mechanically: schema, scope, acceptance metadata, requirement coverage, and checklist structure. This standalone gate, its graph workflow, and its Bash-era command references are historical proposals; they are not a required current authoring step.
- **Current authority:** [task template](../../../../reference/task-template/), [harness authoring and verification guide](../../../../harness/README.md), [experiment design](../../../experiment-design.md), and [research findings](../../RESEARCH-FINDINGS.md).
