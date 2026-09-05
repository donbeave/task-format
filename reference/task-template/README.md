---
schema: task/v4
id: TASK-000
title: "<imperative title: one observable outcome>"
kind: bugfix            # bugfix | feature | refactor | removal | migration | test | docs
verify: "taskfmt verify"
expected_paths:         # scope whitelist: the only paths that may change (bash globs; '*' and '**' cross '/')
  - "src/<area>/*"
  - "tests/<area>/*"       # only if the executor owns these tests; planner-owned acceptance tests live outside this whitelist
---

# TASK-000 — <imperative title>

Execution protocol, progress file grammar, and final report format are in `/task/AGENTS.md`. This file is read-only. It defines WHAT must become true; it does not change during execution.

## Goal

<One sentence. A state that must become true, not a list of actions.>

## Context

Current behavior:

- <Concrete fact about the current implementation or failure.>
- <Concrete consequence.>

Desired behavior:

- <Concrete post-change behavior, observable by a command, request, artifact, or test.>

Read before editing (orientation only, non-normative, in order; never `/task/*`):

1. `<path>` — <why>.
2. `<path>` — <what to understand>.
3. `<path>` — <pattern or contract to follow>.

Code flow: <2-5 sentences on how the named files interact. Define non-obvious terms. A fresh agent must not need any prior chat.>

Baseline (run from repo root, before any edit):

```sh
<AC-001 command>
```

Expected before this change: `<concise failing result>`

## Preconditions

Each precondition has a command (failure handling: see AGENTS.md).

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

Canonical typed acceptance blocks use taskfmt's Markdown profile. Behavior comes first in one
exact `gherkin` fence. A `Verification` section follows with `Type`, `Covers`, and `Expected`
bullets, then one exact command in a fenced `sh` block. Keep a short command on one line. Break
long shell chains after `&&`. Wrap long argument lists with shell continuations and one logical
argument per line. A gate has no `Covers` or Gherkin body. These blocks are task metadata, not
Cucumber feature files and have no runtime step definitions.

### AC-001 — <primary success scenario>
```gherkin
Given the starting state described by the task
When the primary action occurs
Then the required observable outcome is true
```

**Verification**

- **Type:** scenario
- **Covers:** `R-001`
- **Expected:** <exit 0 / output>

```sh
<focused command>
```

### AC-002 — <important edge or failure scenario>
```gherkin
Given the edge or failure state described by the task
When the action is applied
Then the required error or preserved invariant is observable
```

**Verification**

- **Type:** scenario
- **Covers:** `R-002`
- **Expected:** <expected result>

```sh
<focused command part one> &&
<focused command part two>
```

### AC-003 — <non-regression or removal scenario>
```gherkin
Given an existing supported behavior or superseded path
When the changed path is exercised
Then the required behavior is preserved or the old path is absent
```

**Verification**

- **Type:** invariant
- **Covers:** `R-003`
- **Expected:** exit 0, no output

```sh
! <grep command>
```

### AC-004 — Completion gate passes
**Verification**

- **Type:** gate
- **Expected:** exit 0, last line `DONE`

```sh
taskfmt verify
```

## Fixed decisions

Already decided. Implement; do not reopen. Anything not listed here that changes public behavior, architecture, data, or security posture is `NEEDS_REPLAN`, not executor discretion.
Full text: `/task/decisions.md` (binding, read-only) — if present.

- **D-001:** <architecture / API / library / data model>.
- **D-002:** <compatibility or migration policy>.
- **D-003:** <required interface, signature, file location, naming>.

## Checklist

Static plan (grammar and state handling: see AGENTS.md). Every leaf names what becomes true and the evidence that permits checking it; each `AC-*` is cited on the item whose evidence is that AC's command.

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-NNN` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline command run and its failing result recorded in `progress.md` `BASELINE:` — evidence: `<baseline command>` output matches the expected pre-change result.
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** <Coherent unit satisfying `R-001`> — evidence: `<focused command>` exits 0.
        - [ ] **2.1.1** <Sub-step when the unit genuinely decomposes> (`R-001`) — evidence: `<command>` → `<result>`.
        - [ ] **2.1.2** <Sub-step> (`R-001`, `AC-001`) — evidence: `<AC-001 command>` → `<expected>`.
    - [ ] **2.2** <Coherent unit satisfying `R-002`> (`AC-002`) — evidence: `<AC-002 command>` exits 0.
    - [ ] **2.3** Superseded path removed (`R-003`, `R-004`, `AC-003`) — evidence: `! <grep command>` exits 0.
- [ ] **3** Gate passes.
    - [ ] **3.1** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated — evidence: `git status --porcelain` and `git diff --no-renames --stat $TASKFMT_BASE` show only in-scope files.
    - [ ] **3.2** `taskfmt verify` exits 0 with last line `DONE` (`AC-004`) — evidence: final full run (with progress check), full output shown in the transcript.
<!-- checklist:end -->
