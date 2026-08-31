# Archive — task-format README v3.1 experimental

- **Source date:** 2026-08-28.
- **Purpose:** Capture an experimental task-package schema considered before the current control format was built.
- **Durable findings:** Agent work is easier to inspect when a task names one observable outcome, a baseline, narrow scope, fixed decisions, executable acceptance evidence, and a static checklist. Completion must be established by an independent gate, not by the agent's final report.
- **Adopted / superseded:** `task/v4` keeps those principles, with a read-only package, separate derived progress state, typed acceptance blocks, `verify.toml`, and host-side verification. The v3.1 schema, delivery-shape taxonomy, transition policy, mutable package assumptions, and `/task/verify.sh` contract are superseded and must not be copied into live packages.
- **Current authority:** [task template](../../../../reference/task-template/), [execution protocol](../../../../reference/task-template/AGENTS.md), [harness guide](../../../../harness/README.md), and [research findings](../../RESEARCH-FINDINGS.md).
