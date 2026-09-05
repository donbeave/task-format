---
schema: task/v5
id: TASK-003
title: "Create-connection form and the save flow"
kind: feature
---

# TASK-003 — Create-connection form and the save flow

## Goal

Open, edit, validate, save, and render the six-field connection form; return to the list with the new connection selected.

## Context

TASK-002 supplied the store and list. This slice adds D-010 form state, D-032 form keys and validation, runtime save handling, and D-060 form rendering. The list Enter behavior remains deferred to TASK-004.

## Preconditions

- **P-001:** The TASK-002 baseline is available.
- **P-002:** Trusted form, runtime, and screen tests are present.

## Scope

In scope: form state, input handling, save flow, and form rendering. Out of scope: database protocol, browser, custom SQL, grid, store schema, trusted material, and list Enter behavior.

## Requirements

- **R-001:** Implement D-010 `CreateForm`, field order, blank state, and validation.
- **R-002:** Implement D-032 form navigation, editing, port filtering, cancellation, and validation messages.
- **R-003:** Save valid forms, surface duplicate names, and retain form state on failure.
- **R-004:** On successful save reload the list and select the new connection.
- **R-005:** Render the D-060 form, focus, help, and password masking.
- **R-006:** Implement directly without compatibility paths or feature flags.
- **R-007:** Preserve the store contract, trusted material, and deferred features.

## Acceptance criteria

### AC-001 — Form state and keys behave
```gherkin
Given a blank or populated connection form
When form keys are handled
Then fields cycle, edit, filter ports, and cancel as decided
```
**Verification**
- **Type:** scenario
- **Covers:** R-001, R-002
- **Check:** CHK-001

### AC-002 — Save flow reports outcomes
```gherkin
Given valid and duplicate connection forms
When save is requested
Then success reloads and selects while duplicates keep the form open
```
**Verification**
- **Type:** scenario
- **Covers:** R-003, R-004
- **Check:** CHK-002

### AC-003 — Form rendering protects secrets
```gherkin
Given a populated form
When its buffer is rendered
Then labels, focus, help, and masked password behavior hold
```
**Verification**
- **Type:** scenario
- **Covers:** R-005
- **Check:** CHK-003

### AC-004 — Earlier behavior remains green
```gherkin
Given the completed form slice
When its trusted regression suite runs
Then prior and current behavior remain green
```
**Verification**
- **Type:** invariant
- **Covers:** R-001, R-002, R-003, R-004, R-005
- **Check:** CHK-004

### AC-005 — Scope remains protected
```gherkin
Given the candidate
When protection rules run
Then prohibited and unrelated changes are absent
```
**Verification**
- **Type:** invariant
- **Covers:** R-006, R-007
- **Check:** CHK-005

### AC-006 — Package gate passes
**Verification**
- **Type:** gate
- **Check:** CHK-006

## Fixed decisions

- **D-010:** `CreateForm` has Name, Host, Port, Database, User, Password in that order; blank form validates non-empty fields except password and port `1..=65535`.
- **D-011–D-012:** Form save emits `SaveConnection`; runtime returns `Saved`; success reloads the list and selects the new row, while failures retain the form.
- **D-032:** Tab/BackTab wrap focus, printable input edits its field, port accepts digits only, Backspace removes, Esc discards, and Enter reports the decided lowercase validation or duplicate-name error.
- **D-060–D-061:** Draw the deterministic six-line form only through `ui::draw`; show focus and help, and mask every password character.

## Checklist

<!-- checklist:start -->
- [ ] **1** Implement form interaction.
    - [ ] **1.1** Deliver form state and input rules for R-001 and R-002; prove AC-001 via CHK-001.
- [ ] **2** Implement save behavior.
    - [ ] **2.1** Deliver save outcomes for R-003 and R-004; prove AC-002 via CHK-002.
    - [ ] **2.2** Deliver secret-safe rendering for R-005; prove AC-003 via CHK-003.
- [ ] **3** Verify the slice.
    - [ ] **3.1** Preserve behavior for R-001, R-002, R-003, R-004, and R-005; prove AC-004 via CHK-004.
    - [ ] **3.2** Protect scope for R-006 and R-007; prove AC-005 via CHK-005.
    - [ ] **3.3** Complete the package gate for R-006 and R-007; prove AC-006 via CHK-006.
<!-- checklist:end -->
