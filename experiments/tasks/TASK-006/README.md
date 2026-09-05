---
schema: task/v5
id: TASK-006
title: "Custom SQL query screen"
kind: feature
---

# TASK-006 — Custom SQL query screen

## Goal

`x` opens a custom-SQL screen where valid single statements execute through the existing session and render their rows or command result.

## Context

TASK-005 has preview grids but no custom query screen. This slice adds CustomSql state, keys, query validation/runtime flow, and D-060 rendering; disconnect and gallery remain later work.

## Preconditions

TASK-005 focused suites, Docker, toolchain, and trusted custom-SQL tests must be available.

## Scope

In scope: custom SQL UI/state/keys/runtime and database query behavior. Out: disconnect, gallery, protected render/fonts/tests, and frozen preview/store/connect behavior.

## Requirements

- **R-001 (MUST):** Implement D-034 CustomSql entry, editing, cancellation, and retention behavior.
- **R-002 (MUST):** Apply D-025 trimming, single-statement validation, cap, and decided status messages.
- **R-003 (MUST):** Route D-011/D-013 query outcomes through runtime without losing existing results on error.
- **R-004 (MUST):** Execute custom SQL using simple-query and map failures to `DbError::Query`.
- **R-005 (MUST):** Render CustomSql and marker-free result grid per D-060.
- **R-006 (MUST):** Implement final direct design without compatibility or legacy fallback.
- **R-007 (MUST NOT):** Change protected or frozen behavior, use unstable sort, or implement disconnect/gallery.

## Acceptance criteria

### AC-001 — Custom SQL state and validation work
```gherkin
Given a connected browser
When x opens, edits, cancels, or submits custom SQL
Then D-034 input behavior and D-025 validation are observed
```

**Verification**

- **Type:** scenario
- **Covers:** `R-001, R-002, R-003`
- **Check:** `CHK-002`

### AC-002 — Queries and rendering work
```gherkin
Given valid and failing custom statements
When they execute through the session
Then simple-query outcomes and D-060 rendering are observed
```

**Verification**

- **Type:** scenario
- **Covers:** `R-004, R-005`
- **Check:** `CHK-002`

### AC-003 — Prior behavior remains green
```gherkin
Given the trusted TASK-006 series
When regression executes
Then previous task behavior remains green
```

**Verification**

- **Type:** invariant
- **Covers:** `R-001, R-002, R-003, R-004, R-005`
- **Check:** `CHK-003`

### AC-004 — Static quality holds
```gherkin
Given the implementation
When static checks run
Then the direct design and protected scope hold
```

**Verification**

- **Type:** invariant
- **Covers:** `R-006, R-007`
- **Check:** `CHK-004`

### AC-005 — Preconditions hold
```gherkin
Given the task environment
When prerequisite tools and fixtures are checked
Then the task can execute against TASK-005
```

**Verification**

- **Type:** invariant
- **Covers:** `R-006, R-007`
- **Check:** `CHK-001`

### AC-006 — Completion gate passes
**Verification**

- **Type:** gate
- **Check:** `CHK-005`

## Fixed decisions

- **D-025, D-034:** custom SQL validation and interaction.
- **D-011, D-013, D-060:** runtime outcomes and layout.

## Checklist

<!-- checklist:start -->
- [ ] **1** Prepare the slice.
    - [ ] **1.1** Prerequisites and scope hold (`R-006`, `R-007`, `AC-005`, `CHK-001`).
- [ ] **2** Implement custom SQL.
    - [ ] **2.1** Input and validation work (`R-001`, `R-002`, `R-003`, `AC-001`, `CHK-002`).
    - [ ] **2.2** Query and rendering work (`R-004`, `R-005`, `AC-002`, `CHK-002`).
- [ ] **3** Prove the slice.
    - [ ] **3.1** Regression is green (`R-001`, `R-002`, `R-003`, `R-004`, `R-005`, `AC-003`, `CHK-003`).
    - [ ] **3.2** Quality holds (`R-006`, `R-007`, `AC-004`, `CHK-004`).
    - [ ] **3.3** Gate evidence exists (`R-006`, `R-007`, `AC-006`, `CHK-005`).
<!-- checklist:end -->
