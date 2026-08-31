# Decisions

These principles guide the current design. Runtime truth remains in the
harness, configuration, and task packages.

## D-001 — Proof is independent

The executor cannot declare success. A host-side gate reruns the declared
checks after the agent stops.

## D-002 — Inputs are protected

Task instructions and planner-owned verification material stay outside the
agent's editable scope. The implementation cannot rewrite its own contract or
proof.

## D-003 — Scope has a recorded baseline

Each run records its starting commit. Verification compares the result with
that baseline and rejects unauthorized paths.

## D-004 — Runs remain inspectable

Containers, logs, snapshots, commits, and gate records are retained. A missing
record is missing evidence.

## D-005 — Format changes need comparison

A plausible instruction does not become a project rule by assertion. Change one
writing variable, compare it with the control, and retain the result.
