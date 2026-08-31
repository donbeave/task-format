# Experiment design

## Aim

Measure whether a change to task-package writing improves predictable,
verifiable coding-agent work.

## Fair comparison

Keep the software outcome, starting repository, trusted checks, container image,
agent configuration, runtime limits, and host gate fixed. Change one writing
variable at a time.

## Lifecycle

1. Lint the package.
2. Create a fresh clone and record its baseline.
3. Overlay trusted verification material.
4. Run one agent in a persistent container.
5. Run the gate outside the agent.
6. Retain the record and promote only a pass.

## Measures

Compare gate pass rate, false-completion claims, scope violations, diff
stability, retries, and rework across repeated runs. A single favorable run is
not a format result.

See the [research method](research/method.md) and [evidence standard](research/evidence.md).
