---
schema: task/v5
id: TASK-005
title: "Preview grid with client-side sort"
kind: feature
---

# TASK-005 — Preview grid with client-side sort

## Goal

Selecting a browser table fetches its 500-row preview, renders a shared grid, and sorts loaded rows client-side.

## Context

TASK-004 supplies browser tables but no grid. This slice adds preview SQL, `Grid`, sorting, preview runtime flow, and grid rendering. Custom SQL and disconnect remain later work.

## Preconditions

TASK-004 focused suites, Docker, toolchain, and trusted grid tests must be available.

## Scope

In scope: `grid.rs`, grid UI, preview query/session/app/runtime/keys/browser changes. Out: custom SQL, disconnect, gallery, protected render/fonts/tests, and connection/store behavior.

## Requirements

- **R-001 (MUST):** Implement D-050 grid ownership, conversion, and immutable sorted view.
- **R-002 (MUST):** Implement D-051/D-052 stable numeric-or-byte comparison, null placement, sort cycle, and reset rules.
- **R-003 (MUST):** Sort fetched rows only and never re-query.
- **R-004 (MUST):** Build D-025 quoted `SELECT * ... LIMIT 500` and execute it through simple-query.
- **R-005 (MUST):** Implement D-011/D-033 preview state, focused grid navigation, and error preservation.
- **R-006 (MUST):** Render D-060 headers, widths, cursors, nulls, and preview title.
- **R-007 (MUST):** Use final direct design; do not change protected or later-task behavior.

## Acceptance criteria

### AC-001 — Grid sorts fetched rows
```gherkin
Given seeded and synthetic result sets
When the grid sorts a loaded column
Then stable numeric and byte ordering, null placement, and reset rules match D-051 and D-052
```

**Verification**

- **Type:** scenario
- **Covers:** `R-001, R-002, R-003`
- **Check:** `CHK-002`

### AC-002 — Preview flow renders correctly
```gherkin
Given a selected browser table
When its preview succeeds or fails
Then exact SQL, application state, navigation, and rendering follow the decided behavior
```

**Verification**

- **Type:** scenario
- **Covers:** `R-004, R-005, R-006`
- **Check:** `CHK-002`

### AC-003 — Prior behavior remains green
```gherkin
Given the trusted TASK-005 series
When regression executes
Then prior behavior remains green
```

**Verification**

- **Type:** invariant
- **Covers:** `R-001, R-002, R-003, R-004, R-005, R-006`
- **Check:** `CHK-003`

### AC-004 — Static quality holds
```gherkin
Given the implementation
When static checks run
Then protected scope and code quality hold
```

**Verification**

- **Type:** invariant
- **Covers:** `R-007`
- **Check:** `CHK-004`

### AC-005 — Preconditions hold
```gherkin
Given the task environment
When prerequisite tools and fixtures are checked
Then the task can execute against TASK-004
```

**Verification**

- **Type:** invariant
- **Covers:** `R-007`
- **Check:** `CHK-001`

### AC-006 — Completion gate passes
**Verification**

- **Type:** gate
- **Check:** `CHK-005`

## Fixed decisions

- **D-025, D-050–D-053:** preview SQL and frozen grid/sort model.
- **D-011, D-033, D-060:** preview state, keys, and layout.

## Checklist

<!-- checklist:start -->
- [ ] **1** Prepare the slice.
    - [ ] **1.1** Prerequisites and scope hold (`R-007`, `AC-005`, `CHK-001`).
- [ ] **2** Implement preview.
    - [ ] **2.1** Grid sorting works (`R-001`, `R-002`, `R-003`, `AC-001`, `CHK-002`).
    - [ ] **2.2** Preview flow and rendering work (`R-004`, `R-005`, `R-006`, `AC-002`, `CHK-002`).
- [ ] **3** Prove the slice.
    - [ ] **3.1** Regression is green (`R-001`, `R-002`, `R-003`, `R-004`, `R-005`, `R-006`, `AC-003`, `CHK-003`).
    - [ ] **3.2** Quality holds (`R-007`, `AC-004`, `CHK-004`).
    - [ ] **3.3** Gate evidence exists (`R-007`, `AC-006`, `CHK-005`).
<!-- checklist:end -->
