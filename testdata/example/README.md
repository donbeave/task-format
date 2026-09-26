---
schema: task/v5
id: TASK-042
title: "Reject expired refresh tokens before session rotation"
kind: bugfix
---

# TASK-042 — Reject expired refresh tokens before session rotation

## Goal

Expired refresh tokens are rejected before session state rotates.

## Context

`SessionService::rotate` currently starts rotation before expiry validation. The refresh endpoint
must reject expired tokens without writing session state while preserving valid-token rotation.

## Preconditions

- **P-001:** The refresh-token contract fixture exists.
- **P-002:** Rust tooling is available.

## Scope

In scope:

- Refresh-token expiry ordering, HTTP error mapping, and direct tests.

Out of scope:

- Access-token expiry and session-storage redesign.

## Requirements

- **R-001 (MUST):** Validate expiry before the rotation transaction begins.
- **R-002 (MUST):** Return HTTP 401 with `refresh_token_expired` for an expired refresh token.
- **R-003 (MUST NOT):** Write session state on the expired path.
- **R-004 (MUST):** Preserve valid refresh-token rotation.
- **R-005 (MUST):** Remove `legacy_expiry_check`.
- **R-006 (MUST):** The completion gate succeeds.

## Acceptance criteria

### AC-001 — Expired refresh is rejected
```gherkin
Given an existing session and an expired refresh token
When the refresh endpoint receives the token
Then it returns 401 with error code refresh_token_expired
And no session state changes
```

**Verification**

- **Type:** scenario
- **Covers:** `R-001, R-002, R-003`
- **Check:** `CHK-001`

### AC-002 — Valid refresh still rotates
```gherkin
Given a valid refresh token
When the refresh endpoint receives the token
Then rotation succeeds
```

**Verification**

- **Type:** scenario
- **Covers:** `R-004`
- **Check:** `CHK-002`

### AC-003 — Legacy helper is absent
```gherkin
Given the completed repository
When the source tree is inspected
Then legacy_expiry_check is absent
```

**Verification**

- **Type:** invariant
- **Covers:** `R-005`
- **Check:** `CHK-003`

### AC-004 — Completion gate passes

**Verification**

- **Type:** gate
- **Check:** `CHK-004`

## Fixed decisions

- **D-001:** Use the existing `TokenStore::validate` implementation.
- **D-002:** The error code is exactly `refresh_token_expired`.

## Checklist

<!-- checklist:start -->
- [ ] **1** Reproduce.
    - [ ] **1.1** Confirm expired-token behavior. (`R-001`, `R-002`, `R-003`, `AC-001`, `CHK-001`)
- [ ] **2** Implement.
    - [ ] **2.1** Reject expired tokens before state changes. (`R-001`, `R-002`, `R-003`, `AC-001`, `CHK-001`)
    - [ ] **2.2** Preserve valid-token rotation. (`R-004`, `AC-002`, `CHK-002`)
    - [ ] **2.3** Remove the legacy helper. (`R-005`, `AC-003`, `CHK-003`)
- [ ] **3** Verify.
    - [ ] **3.1** Run the completion gate. (`R-006`, `AC-004`, `CHK-004`)
<!-- checklist:end -->
