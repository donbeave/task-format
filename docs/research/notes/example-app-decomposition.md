# Archive — example app decomposition

- **Source date:** 2026-08-29.
- **Purpose:** A planning proposal for using a terminal PostgreSQL client (`pgtui`) as a realistic task-format fixture.
- **Status:** Historical design note. It does not define the current experiment, package format, or implementation plan.

## Durable findings

- Split agent work by independently verifiable user outcomes, not by technical layers.
- Resolve consequential design decisions before dispatch; a fresh agent should implement a contract, not invent one.
- Give every task a single completion gate with focused, inspectable evidence.
- Keep reusable verification fixtures planner-owned and outside the agent's editable scope.
- For interactive interfaces, test a deterministic representation; do not treat platform-dependent pixels as a portable oracle.

## Adopted and superseded

The project adopted the underlying fixture idea and the principle of small, ordered tasks. The current control uses seven `pgtui` task packages and a Rust harness.

The proposed six-task application architecture, dependency choices, exact key bindings, Bash-era protected manifests, and snapshot layout are superseded. They are useful only as provenance for the questions the current format addresses.

## Current authority

- [Research findings](../RESEARCH-FINDINGS.md) — current conclusions and open evidence.
- [Experiment design](../../experiment-design.md) — controlled-run method.
- [Task-package reference](../../../reference/task-template/) — current agent-visible format.
- [Experiment packages](../../../experiments/) and [harness guide](../../../harness/README.md) — current executable behavior.
