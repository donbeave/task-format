---
schema: task/v5
id: TASK-007
title: "Disconnect, exit codes and the screen gallery"
kind: feature
---

# TASK-007 — Disconnect, exit codes and the screen gallery

## Goal

Connected sessions disconnect cleanly, interactive exits restore the terminal with exit zero, and gallery deterministically publishes the ten decided screen pairs and README index.

## Context

TASK-006 completes query surfaces but leaves disconnect and gallery stubbed. This release slice finishes teardown, terminal exit behavior, and deterministic gallery artifacts.

## Preconditions

TASK-006 custom-SQL suite, Docker, toolchain, and trusted gallery tests must be available.

## Scope

In scope: disconnect state/socket teardown, exits, gallery binary, screens, and README Screens section. Out: frozen store/connect/preview/custom-SQL behavior, protected render/fonts/tests, CI, packaging, and theming.

## Requirements

- **R-001 (MUST):** `d` emits disconnect and `Msg::Disconnected` resets session/grids/input while retaining saved connections and list cursor.
- **R-002 (MUST):** Drop the client and await its connection task before reporting disconnection.
- **R-003 (MUST):** `q` and Ctrl+C restore terminal and exit zero; Ctrl+C disconnects before quit while connected.
- **R-004 (MUST):** Gallery implements D-080 names, deterministic SVG/PNG pairs, prescribed output and exit codes, and reuses rendering.
- **R-005 (MUST):** Commit one default gallery run and list all ten PNG paths under README `## Screens`.
- **R-006 (MUST):** Implement final direct design without compatibility or legacy fallback.
- **R-007 (MUST NOT):** Change protected/frozen behavior, use unstable sort, or create unrelated tooling.

## Acceptance criteria

### AC-001 — Disconnect and exits work
```gherkin
Given a connected application in a real terminal
When d, q, or Ctrl+C are applied
Then state resets, backend teardown completes, and terminal exit behavior follows D-012, D-030, and D-040
```

**Verification**

- **Type:** scenario
- **Covers:** `R-001, R-002, R-003`
- **Check:** `CHK-002`

### AC-002 — Gallery release artifacts work
```gherkin
Given the gallery binary
When it runs twice with default or explicit output
Then ten deterministic SVG and PNG pairs and the Screens index meet D-080
```

**Verification**

- **Type:** scenario
- **Covers:** `R-004, R-005`
- **Check:** `CHK-002`

### AC-003 — Full release regression remains green
```gherkin
Given the trusted final series
When regression executes
Then all earlier behavior remains green
```

**Verification**

- **Type:** invariant
- **Covers:** `R-001, R-002, R-003, R-004, R-005`
- **Check:** `CHK-003`

### AC-004 — Static quality holds
```gherkin
Given the release implementation
When static checks run
Then protected scope and direct-design rules hold
```

**Verification**

- **Type:** invariant
- **Covers:** `R-006, R-007`
- **Check:** `CHK-004`

### AC-005 — Preconditions hold
```gherkin
Given the release environment
When prerequisite tools and fixtures are checked
Then TASK-007 can execute against TASK-006
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

- **D-024, D-033:** disconnect and teardown.
- **D-012, D-030, D-040:** loop and terminal exits.
- **D-080, D-005:** gallery and Screens index.

## Checklist

<!-- checklist:start -->
- [ ] **1** Prepare release validation.
    - [ ] **1.1** Prerequisites and scope hold (`R-006`, `R-007`, `AC-005`, `CHK-001`).
- [ ] **2** Complete release behavior.
    - [ ] **2.1** Disconnect and exits work (`R-001`, `R-002`, `R-003`, `AC-001`, `CHK-002`).
    - [ ] **2.2** Gallery artifacts and index work (`R-004`, `R-005`, `AC-002`, `CHK-002`).
- [ ] **3** Prove release completion.
    - [ ] **3.1** Regression is green (`R-001`, `R-002`, `R-003`, `R-004`, `R-005`, `AC-003`, `CHK-003`).
    - [ ] **3.2** Quality holds (`R-006`, `R-007`, `AC-004`, `CHK-004`).
    - [ ] **3.3** Gate evidence exists (`R-006`, `R-007`, `AC-006`, `CHK-005`).
<!-- checklist:end -->
