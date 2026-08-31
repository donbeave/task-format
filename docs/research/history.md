# History

Early work explored task templates, checklists, readiness checks, verification,
container execution, agent tooling, and the `pgtui` example. The work converged
on five durable ideas:

1. Give one agent one bounded, independently verifiable outcome.
2. Resolve material design choices before dispatch.
3. Keep the contract and its proof outside the agent's control.
4. Use progress files for resumption, never as the success verdict.
5. Retain a host-side gate and run evidence for every result.

Superseded drafts and raw task artifacts were removed from the active research
record because they mixed old package shapes with current guidance. Their
durable conclusions are represented in [method](method.md),
[evidence](evidence.md), and [decisions](decisions.md).
