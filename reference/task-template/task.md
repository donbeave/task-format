---
schema: task/v3
id: TASK-000
title: "<imperative title: one observable outcome>"
kind: bugfix            # bugfix | feature | refactor | removal | migration | test | docs
verify: "/task/verify.sh"
expected_paths:         # orientation for the agent; scope metric for the harness (bash globs)
  - "src/<area>/*"
  - "tests/<area>/*"
protected_paths:        # hashed at dispatch; any change fails the gate
  - "tests/fixtures/<file>"
---

# TASK-000 — <imperative title>

Execution protocol, progress file grammar, and final report format are in `/task/AGENT.md`. This file is read-only. It defines WHAT must become true; it does not change during execution.

## Goal

<One sentence. A state that must become true, not a list of actions.>

## Context

Current behavior:

- <Concrete fact about the current implementation or failure.>
- <Concrete consequence.>

Desired behavior:

- <Concrete post-change behavior, observable by a command, request, artifact, or test.>

Read before editing (non-normative hints, in order):

1. `<path>` — <why>.
2. `<path>` — <what to understand>.
3. `<path>` — <pattern or contract to follow; already decided, do not reopen>.

Code flow: <2-5 sentences on how the named files interact. Define non-obvious terms. A fresh agent must not need any prior chat.>

Baseline (run from repo root, before any edit):

```sh
<exact command>
```

Expected before this change: `<concise failing result>`

## Preconditions

Each precondition has a command. If a command fails, stop and report `BLOCKED` (see AGENT.md). Do not work around it.

- **P-001:** <state> — `<command that exits 0 when true>`
- **P-002:** <state> — `<command>`

## Scope

In scope:

- <One coherent behavior or vertical slice.>
- <Tests directly proving it.>
- <Removal of the superseded implementation.>

Out of scope:

- <Adjacent feature or independent improvement.>
- <Unrelated cleanup, dependency upgrades, formatting sweeps, speculative abstractions.>

## Requirements

- **R-001 (MUST):** <required behavior or interface>.
- **R-002 (MUST):** <required error handling, edge case, or invariant>.
- **R-003 (MUST NOT):** <forbidden behavior or shortcut>.
- **R-004 (MUST):** Implement the final design directly. No compatibility layer, dual path, deprecated alias, feature flag, or legacy fallback unless a requirement above demands one.

## Acceptance criteria

Each row is observable behavior with the exact evidence command. `verify.sh` runs these; the harness re-runs them.

| ID | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- |
| AC-001 | Given <state>, when <action>, then <observable result>. | `<command>` | `<exit 0 / output>` |
| AC-002 | Given <state>, when <edge or failure>, then <error or preserved invariant>. | `<command>` | `<expected>` |
| AC-003 | Given <old path>, when <changed path exercised>, then <preserved behavior or old path absent>. | `<command>` | `<expected>` |

## Fixed decisions

Already decided. Implement; do not reopen. Anything not listed here that changes public behavior, architecture, data, or security posture is `NEEDS_REPLAN`, not executor discretion.

- **D-001:** <architecture / API / library / data model>.
- **D-002:** <compatibility or migration policy>.
- **D-003:** <required interface, signature, file location, naming>.

## Checklist

Static plan. Hierarchical IDs `N`, `N.N`, `N.N.N`, `N.N.N.N` (max depth 4, four spaces per level). Every leaf names what becomes true and the evidence that permits checking it. State is tracked in `progress.md`, never here.

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-NNN` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline command run and its failing result recorded in `progress.md` `BASELINE:` — evidence: `<baseline command>` output matches the expected pre-change result.
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** <Coherent unit satisfying `R-001`> — evidence: `<focused command>` exits 0.
        - [ ] **2.1.1** <Sub-step when the unit genuinely decomposes> (`R-001`) — evidence: `<command>` → `<result>`.
        - [ ] **2.1.2** <Sub-step> (`R-001`, `AC-001`) — evidence: `<command>` → `<result>`.
    - [ ] **2.2** <Coherent unit satisfying `R-002`> — evidence: `<command>` exits 0.
    - [ ] **2.3** Superseded path removed (`R-003`, `R-004`) — evidence: `<grep command>` prints nothing and regressions pass.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001` — evidence: `<AC-001 command>` → `<expected>`.
    - [ ] **3.2** `AC-002` — evidence: `<AC-002 command>` → `<expected>`.
    - [ ] **3.3** `AC-003` — evidence: `<AC-003 command>` → `<expected>`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated — evidence: `git status --porcelain` and `git diff --stat` show only in-scope files.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full verifier output shown in the transcript.
<!-- checklist:end -->
