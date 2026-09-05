---
schema: task/v5
id: TASK-002
title: "Connection store, connection list screen and CLI skeleton"
kind: feature
---

# TASK-002 — Connection store, connection list screen and CLI skeleton

## Goal

Persist connections locally, render the sorted cursor-driven list, and run the list from the CLI.

## Context

TASK-001 provided only the scaffold. This slice adds the store, state machine, keys, rendering, runtime loop, and CLI. Trusted behavioral tests define their observable contracts.

## Preconditions

- **P-001:** The TASK-001 workspace baseline is available.
- **P-002:** Trusted store, application, screen, and CLI tests are present.

## Scope

In scope: connection persistence, list state and keys, list/status rendering, CLI loop, and required module declarations. Out of scope: create form, database protocol, browser, grid, keyring, and trusted material.

## Requirements

- **R-001:** Implement D-020 through D-022 store path selection, schema, exact API, sorted listing, and duplicate-name error.
- **R-002:** Implement the D-010/D-011 application state, messages, effects, and placeholders.
- **R-003:** Implement D-030/D-031 list navigation, creation entry, quit, and deferred Enter behavior.
- **R-004:** Render D-060/D-061 connection-list and status behavior without secrets.
- **R-005:** Implement D-012/D-040/D-042 CLI diagnostics, loop, and terminal restoration.
- **R-006:** Implement directly without compatibility paths or feature flags.
- **R-007:** Preserve pins and trusted material; do not add deferred application features.

## Acceptance criteria

### AC-001 — Connections persist and sort
```gherkin
Given a fresh local database
When named connections are saved and reopened
Then they persist in ascending name order
```
**Verification**
- **Type:** scenario
- **Covers:** R-001
- **Check:** CHK-001

### AC-002 — List state and keys behave
```gherkin
Given saved connections in the list
When navigation and quit keys are handled
Then selection clamps and quit emits its effect
```
**Verification**
- **Type:** scenario
- **Covers:** R-002, R-003
- **Check:** CHK-002

### AC-003 — List rendering is safe
```gherkin
Given populated and empty connection lists
When buffers are rendered
Then list, selection, guidance, status, and secret redaction hold
```
**Verification**
- **Type:** scenario
- **Covers:** R-004
- **Check:** CHK-003

### AC-004 — CLI behavior holds
```gherkin
Given CLI diagnostics and an interactive list session
When the process is invoked or quit
Then decided exits and terminal restoration hold
```
**Verification**
- **Type:** scenario
- **Covers:** R-005
- **Check:** CHK-004

### AC-005 — Earlier behavior remains green
```gherkin
Given the completed slice
When its trusted regression suite runs
Then prior and current behavior remain green
```
**Verification**
- **Type:** invariant
- **Covers:** R-001, R-002, R-003, R-004, R-005
- **Check:** CHK-005

### AC-006 — Scope remains protected
```gherkin
Given the candidate
When protection rules run
Then prohibited and unrelated changes are absent
```
**Verification**
- **Type:** invariant
- **Covers:** R-006, R-007
- **Check:** CHK-006

### AC-007 — Package gate passes
**Verification**
- **Type:** gate
- **Check:** CHK-007

## Fixed decisions

- **D-010–D-013:** Application state, effects, loop, and status semantics.
- **D-020–D-022:** Turso store and exact API.
- **D-030–D-031:** Global and connection-list keys.
- **D-040–D-042:** Exit diagnostics and non-interactive behavior.
- **D-060–D-061:** List layout and drawing entry point.

## Checklist

<!-- checklist:start -->
- [ ] **1** Implement persistence.
    - [ ] **1.1** Deliver sorted durable storage for R-001; prove AC-001 via CHK-001.
- [ ] **2** Implement interaction.
    - [ ] **2.1** Deliver app state and keys for R-002 and R-003; prove AC-002 via CHK-002.
    - [ ] **2.2** Deliver safe list rendering for R-004; prove AC-003 via CHK-003.
    - [ ] **2.3** Deliver CLI behavior for R-005; prove AC-004 via CHK-004.
- [ ] **3** Verify the slice.
    - [ ] **3.1** Preserve prior behavior for R-001, R-002, R-003, R-004, and R-005; prove AC-005 via CHK-005.
    - [ ] **3.2** Protect scope for R-006 and R-007; prove AC-006 via CHK-006.
    - [ ] **3.3** Complete the package gate for R-006 and R-007; prove AC-007 via CHK-007.
<!-- checklist:end -->
