---
schema: task/v5
id: TASK-004
title: "Connect to PostgreSQL and list its tables"
kind: feature
---

# TASK-004 — Connect to PostgreSQL and list its tables

## Goal

Opening a saved connection creates a live PostgreSQL session and lists base tables in the browser sidebar; failures remain status errors on the connection list.

## Context

TASK-003 supplies the connection form but no session or browser. This slice introduces the simple-query session, connect runtime effect, and browser sidebar. Preview, custom SQL, disconnect, and gallery remain later work.

## Preconditions

The toolchain, TASK-003 library build, Docker, and trusted seed fixtures must be available.

## Scope

In scope: database types/session, connect runtime effect, browser state and rendering, and required module/dependency declarations. Out: preview, custom SQL, disconnect, gallery, render pipeline, fonts, trusted tests, and store contract.

## Requirements

- **R-001 (MUST):** Provide D-026 database types, `PREVIEW_LIMIT = 500`, `Cell`, `ResultSet`, `TableRef` display as `schema.name`, `DbError`, and `quote_ident`.
- **R-002 (MUST):** Use `Client::simple_query` for every database statement; no query, prepare, or execute API appears under `src/db`.
- **R-003 (MUST):** Build PostgreSQL config from `ConnParams` with `application_name=pgtui`, `NoTls`, and a five-second timeout mapped to `DbError::Timeout`; failed connection is non-fatal.
- **R-004 (MUST):** List tables with D-025 exact SQL ordered by `table_schema, table_name`.
- **R-005 (MUST):** Implement D-033/D-060 browser navigation, focus, sidebar markers, and empty-grid main title.
- **R-006 (MUST):** Implement final design directly without compatibility or legacy fallback.
- **R-007 (MUST NOT):** Change protected pins/render/fonts/tests or implement later-task behavior.

## Acceptance criteria

### AC-001 — Connection and table behavior
```gherkin
Given a seeded PostgreSQL container
When a saved connection opens
Then session errors are mapped and base tables are listed in decided order
```

**Verification**

- **Type:** scenario
- **Covers:** `R-001, R-002, R-003, R-004`
- **Check:** `CHK-002`

### AC-002 — Browser behavior
```gherkin
Given a successful or failed connection result
When browser state and rendering are exercised
Then transitions, sidebar navigation, focus, and markers follow D-033 and D-060
```

**Verification**

- **Type:** scenario
- **Covers:** `R-005`
- **Check:** `CHK-002`

### AC-003 — Prior behavior remains green
```gherkin
Given the trusted TASK-004 series
When regression executes
Then prior connection-list and form behavior remains green
```

**Verification**

- **Type:** invariant
- **Covers:** `R-001, R-002, R-003, R-004, R-005`
- **Check:** `CHK-003`

### AC-004 — Static quality holds
```gherkin
Given the implementation
When formatting and lint run
Then the workspace has no formatting or lint violations
```

**Verification**

- **Type:** invariant
- **Covers:** `R-006, R-007`
- **Check:** `CHK-004`

### AC-005 — Preconditions hold
```gherkin
Given the task environment
When required tools and trusted fixtures are checked
Then the task can be executed against its stated predecessor
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

- **D-023–D-026:** All PostgreSQL statements use `Client::simple_query`; query/prepare/execute APIs are forbidden. Connect with decided field config, `application_name=pgtui`, `NoTls`, and a five-second timeout. Keep the decided database types, identifier quoting, `PREVIEW_LIMIT = 500`, and ordered base-table SQL.
- **D-033:** Browser sidebar navigation clamps; Tab toggles sidebar/grid only when a grid exists; Enter starts the selected-table preview only in its owning later slice.
- **D-060:** Draw the 100x30 browser through `ui::draw`: 30-column table sidebar, focus marker, `schema.name` rows, and an empty main panel titled by connection.

## Checklist

<!-- checklist:start -->
- [ ] **1** Establish the task environment.
    - [ ] **1.1** Environment and protected scope are ready (`R-006`, `R-007`, `AC-005`, `CHK-001`).
- [ ] **2** Implement the connection slice.
    - [ ] **2.1** Database types, connection, and table listing work (`R-001`, `R-002`, `R-003`, `R-004`, `AC-001`, `CHK-002`).
    - [ ] **2.2** Browser state and rendering work (`R-005`, `AC-002`, `CHK-002`).
- [ ] **3** Prove completion.
    - [ ] **3.1** Regression remains green (`R-001`, `R-002`, `R-003`, `R-004`, `R-005`, `AC-003`, `CHK-003`).
    - [ ] **3.2** Static quality holds (`R-006`, `R-007`, `AC-004`, `CHK-004`).
    - [ ] **3.3** Gate evidence is recorded (`R-006`, `R-007`, `AC-006`, `CHK-005`).
<!-- checklist:end -->
