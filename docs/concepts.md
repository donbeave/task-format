# Core concepts

## Task package

The versioned input handed to an agent for one bounded change. It states the work to perform, the execution protocol, and how completion is checked. The package is read-only during a run.

## Fresh run

One agent, one fresh container, and one fresh clone of the target experiment repository. Reusing an old workspace would mix prior state with the format being tested.

## Baseline

The recorded commit that represents the repository state immediately before an agent begins work. The harness records it so verification and scope checks refer to a fixed point.

## Trusted material

Planner-owned verification support supplied separately from the agent’s editable work. The harness overlays it before execution and treats it as part of the run baseline.

## Scope

The paths an agent may change for a task. Scope is enforced by the gate against the recorded baseline, so a passing implementation must stay within the authorized change surface.

## Machine gate

Independent verification run by the harness after the agent stops. The gate checks declared commands, scope, and completion state. Agent reports are useful context, not verdicts.

## Promotion

The only path from a completed run to the experiment repository’s main branch. Promotion is available only after the host-side gate records a pass.

## Run record

The persistent artifacts for one execution: configuration, task snapshot, baseline and result commits, logs, container state, and gate verdict. These records make a run inspectable after the fact.

## Lifecycle

1. Prepare and lint a task package.
2. Create fresh repository state and record its baseline.
3. Add trusted verification material to that baseline.
4. Start one agent in a persistent, inspectable container.
5. Run the machine gate from trusted host-side inputs.
6. Promote only a passing result; retain every run record either way.
