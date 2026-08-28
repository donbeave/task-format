---
schema: task/v3.1-experimental
id: TASK-000
title: "<imperative title: one observable outcome>"
kind: bugfix            # bugfix | feature | refactor | removal | migration | test | docs
shape: tracer-bullet    # tracer-bullet | prefactor | wide-refactor-expand | wide-refactor-migrate | wide-refactor-contract | integration
verify: "/task/verify.sh"
expected_paths:
  - "src/<area>/*"
  - "tests/<area>/*"
protected_paths:
  - "tests/fixtures/<file>"
---

# TASK-000 — <imperative title>

Execution protocol, progress grammar, and final report format are in
`/task/AGENTS.md`. This file and every binding source under `/task` are
read-only. They define what must become true; they do not change during
execution.

## Goal

<One sentence describing one final observable state, not a list of work.>

**Demo path:** `<one command or interaction>` → `<observable result>`.

**Independent value:** <what remains useful and verifiable if no later task is
implemented>.

## Context

Current behavior:

- <Concrete current fact or failure.>
- <Concrete consequence.>

Desired behavior:

- <Concrete post-change behavior.>

**Primary verification seam:** `<highest stable public/system boundary>`  
**Why this seam:** <why it proves the outcome without coupling to internals>  
**Supporting seams:** `none` or <bounded seams required to inspect otherwise
invisible invariants, with rationale>  
**Prior art:** `<existing test/helper/reference implementation>`

Binding sources (read-only; contradiction means `NEEDS_REPLAN`):

1. `/task/decisions.md` — <imported D IDs and provenance>, if present.
2. `<ADR/spec/contract snapshot>` — <binding content>.

Orientation hints (non-normative, in order):

1. `<path>` — <why to read it>.
2. `<path>` — <current flow or existing pattern>.
3. `<test path>` — <prior art for the agreed seam>.

Code flow: <2–5 sentences sufficient for a fresh agent.>

Baseline (run from repo root before any edit):

```sh
<exact command>
```

Expected before this change: `<concise distinguishing result>`.

## Preconditions

Each precondition has a command. A failed dependency/environment precondition
is `BLOCKED`; a stale or contradictory contract is `NEEDS_REPLAN`.

- **P-001:** <state> — `<command that exits 0 when true>`
- **P-002:** <state> — `<command>`

## Scope

**Delivery shape:** `<explain why this is a tracer bullet, prefactor, or one
bounded role in an approved wide-refactor sequence>`.

This task owns every layer required by its acceptance criteria. No criterion
depends on an unfinished task.

In scope:

- <One coherent, independently demonstrable behavior or structural outcome.>
- <Every layer and test required to prove it.>
- <Removal of superseded behavior when this is a direct transition.>

Out of scope:

- <Real adjacent behavior deliberately refused.>
- <Work owned by another task.>
- <Unrelated cleanup, upgrades, formatting sweeps, speculative abstractions.>

For a wide-refactor task, state the exact bounded transition role, completed
predecessors, mandatory contract/removal task, and temporary coexistence
allowed. Otherwise require the final design directly with no dual path.

## Requirements

- **R-001 (MUST):** <required observable behavior or interface>.
- **R-002 (MUST):** <error case, edge case, or invariant>.
- **R-003 (MUST NOT):** <forbidden behavior or shortcut>.
- **R-004 (MUST):** <instantiate one of the following, not both>:
  - Direct transition: implement the final design directly; no compatibility
    layer, dual path, deprecated alias, feature flag, or legacy fallback.
  - Approved bounded transition: implement exactly `<expand|migrate|contract>`
    role from `<graph/task IDs>`; do not extend the coexistence boundary; the
    mandatory contract task is `<TASK-ID>`.

## Acceptance criteria

Every criterion names its class and its observation at the exact base commit.

- `delta`: false/old at baseline, required behavior true finally.
- `invariant`: supported behavior passes at baseline and finally.
- `removal`: old artifact/behavior present at baseline and absent finally.

| ID | Class | Given / When / Then | Evidence command | Baseline expected | Final expected |
| --- | --- | --- | --- | --- | --- |
| AC-001 | delta | Given <state>, when <action>, then <new observable result>. | `<command>` | `<failure or old output>` | `<exit 0 / new output>` |
| AC-002 | invariant | Given <supported state>, when <action>, then <preserved result>. | `<command>` | `<pass/result>` | `<same pass/result>` |
| AC-003 | removal | Given <old path>, when inspected/exercised, then it is absent. | `<command>` | `<present/old behavior>` | `<absent/new behavior>` |

## Fixed decisions

Every decision was made before task compilation. Do not invent a value merely
to fill the table.

| ID | Decision | Provenance |
| --- | --- | --- |
| D-001 | <architecture/API/library/data decision> | `<ADR/spec/operator decision>` |
| D-002 | <compatibility/migration policy> | `<source>` |
| D-003 | <required interface/signature/path/name> | `<source>` |

Anything not listed here that changes public behavior, architecture, data,
security posture, transition policy, or verification seam is
`NEEDS_REPLAN`.

## Checklist

Static plan. IDs `N`…`N.N.N.N`, maximum depth four, four spaces per level.
Every leaf names what becomes true and the evidence that permits checking it.
State is tracked in `progress.md`, never here.

<!-- checklist:start -->
- [ ] **1** Starting state is proven.
    - [ ] **1.1** Preconditions `P-001..P-NNN` pass — evidence: each command exits 0.
    - [ ] **1.2** Baseline command and AC baseline observations are recorded in `progress.md` — evidence: every `delta`/`removal` criterion shows its declared pre-change state and every `invariant` criterion shows its declared baseline result.
- [ ] **2** Bounded deliverable is implemented.
    - [ ] **2.1** <Coherent outcome satisfying R/D IDs> — evidence: `<focused command>` → `<result>`.
        - [ ] **2.1.1** <Meaningful sub-outcome, only if genuinely needed> — evidence: `<command>` → `<result>`.
        - [ ] **2.1.2** <Meaningful sub-outcome> — evidence: `<command>` → `<result>`.
    - [ ] **2.2** <Next task-local outcome> — evidence: `<command>` → `<result>`.
    - [ ] **2.3** <Required cleanup or bounded transition obligation> — evidence: `<command>` → `<result>`.
- [ ] **3** Observable contract is proven at the agreed seams.
    - [ ] **3.1** `AC-001` at the primary seam — evidence: `<AC-001 command>` → `<final expected>`.
    - [ ] **3.2** `AC-002` invariant — evidence: `<AC-002 command>` → `<final expected>`.
    - [ ] **3.3** `AC-003` removal — evidence: `<AC-003 command>` → `<final expected>`.
    - [ ] **3.4** Demo path succeeds — evidence: `<demo command/artifact>` → `<observable result>`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only `expected_paths` changed and no temporary, unrelated, or unbounded transition work remains — evidence: `git status --porcelain` and `git diff --stat`.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full verifier output shown in the transcript.
<!-- checklist:end -->
