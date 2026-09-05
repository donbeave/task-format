---
schema: task/v5
id: TASK-000
title: "<imperative title>"
kind: bugfix
---

# TASK-000 — <imperative title>

## Goal

<One observable state that must become true.>

## Context

<Current behavior, desired behavior, and the smallest relevant code-flow context.>

## Preconditions

- **P-001:** <required starting state.>

## Scope

In scope:

- <One coherent behavior and its direct proof.>

Out of scope:

- <Unrelated change.>

## Requirements

- **R-001 (MUST):** <primary required behavior.>
- **R-002 (MUST):** <required edge case or invariant.>
- **R-003 (MUST NOT):** <forbidden behavior or superseded path.>
- **R-004 (MUST):** <required non-regression.>
- **R-005 (MUST):** The completion gate succeeds.

## Acceptance criteria

### AC-001 — <primary success scenario>
```gherkin
Given <starting state>
When <primary action>
Then <observable outcome>
```

**Verification**

- **Type:** scenario
- **Covers:** `R-001`
- **Check:** `CHK-001`

### AC-002 — <edge scenario>
```gherkin
Given <edge state>
When <action>
Then <preserved invariant>
```

**Verification**

- **Type:** scenario
- **Covers:** `R-002`
- **Check:** `CHK-002`

### AC-003 — <forbidden path is absent>
```gherkin
Given <completed change>
When <source is inspected>
Then <forbidden behavior is absent>
```

**Verification**

- **Type:** invariant
- **Covers:** `R-003`
- **Check:** `CHK-003`

### AC-004 — <supported behavior remains>
```gherkin
Given <supported behavior>
When <change is exercised>
Then <behavior remains correct>
```

**Verification**

- **Type:** scenario
- **Covers:** `R-004`
- **Check:** `CHK-004`

### AC-005 — Completion gate passes

**Verification**

- **Type:** gate
- **Check:** `CHK-005`

## Fixed decisions

- **D-001:** <binding implementation decision.>

## Checklist

<!-- checklist:start -->
- [ ] **1** Prepare.
    - [ ] **1.1** Confirm the starting state. (`R-001`, `AC-001`, `CHK-001`)
- [ ] **2** Implement.
    - [ ] **2.1** Deliver primary behavior. (`R-001`, `AC-001`, `CHK-001`)
    - [ ] **2.2** Deliver edge behavior. (`R-002`, `AC-002`, `CHK-002`)
    - [ ] **2.3** Remove the forbidden path. (`R-003`, `AC-003`, `CHK-003`)
    - [ ] **2.4** Preserve supported behavior. (`R-004`, `AC-004`, `CHK-004`)
- [ ] **3** Verify.
    - [ ] **3.1** Run the completion gate. (`R-005`, `AC-005`, `CHK-005`)
<!-- checklist:end -->
