# Method

## Question

For one bounded software outcome, which task-package structure produces the
most predictable agent result?

## Comparison unit

One observation is one task run with:

- one task package;
- one fresh clone and recorded baseline;
- one fresh, persistent container and agent session;
- one independent host-side completion gate; and
- a retained run record.

## Experimental rule

Change one meaningful writing variable per comparison. Candidate variables may
include checklist shape, rule placement, context order, or how verification is
presented. Do not bundle format changes with changes to the software outcome,
tests, or runtime.

## Controls

Keep these fixed within a comparison:

- target outcome and authorized scope;
- starting repository and trusted verification material;
- gate commands and pass condition;
- container image, agent profile, and runtime limits; and
- run setup and artifact retention.

## Valid comparison

A result is usable only when the gate can decide it, the controls were held,
and another investigator can inspect the run artifacts. One successful run is a
signal for follow-up, not a basis for changing the control format.
