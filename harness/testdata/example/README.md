---
schema: task/v4
id: TASK-042
title: "Reject expired refresh tokens before session rotation"
kind: bugfix
verify: "taskfmt verify"
expected_paths:
  - "src/auth/session/*"
  - "tests/auth/*"
  - "Cargo.lock"
---

# TASK-042 — Reject expired refresh tokens before session rotation

Execution protocol, progress file grammar, and final report format are in `/task/AGENTS.md`. This file is read-only.

## Goal

Expired refresh tokens are rejected before any session state is rotated.

## Context

Current behavior:

- `SessionService::rotate` in `src/auth/session/rotate.rs` begins a transaction and increments `rotation_counter` before `TokenStore::validate` checks expiry.
- An expired token therefore leaves a rotated counter and an orphan session row, and the client receives a generic 500.

Desired behavior:

- `POST /auth/refresh` with an expired token returns HTTP 401, body `{"error":"refresh_token_expired"}`, and no session row, counter, or replacement token is written.

Read before editing (orientation only, non-normative, in order):

1. `AGENTS.md` — test and lint commands.
2. `src/auth/session/rotate.rs` — the rotation flow; the ordering bug is here.
3. `src/auth/token_store.rs` — `validate()` already returns `TokenError::Expired`; reuse it.
4. `docs/decisions/D-041.md` — error-code contract.

Code flow: the HTTP handler in `src/auth/http/refresh.rs` calls `SessionService::rotate(token)`. `rotate` opens a DB transaction, increments the counter, calls `TokenStore::validate`, then issues a new token. The expiry check must move before the transaction opens. The legacy `legacy_expiry_check` helper in `rotate.rs` duplicates `validate` and must go.

Baseline (run from repo root, before any edit):

```sh
cargo test -p auth expired_refresh_token
```

Expected before this change: `1 failed` — `rotation_counter == 1, want 0`.

## Preconditions

- **P-001:** contract fixture present — `test -f tests/fixtures/refresh-token-contract.json`
- **P-002:** toolchain available — `cargo --version`

## Scope

In scope:

- Refresh-token expiry validation ordering in `SessionService::rotate`.
- The `refresh_token_expired` error mapping in the refresh handler.
- Tests proving the failure path and the unchanged success path.
- Removal of `legacy_expiry_check`.

Out of scope:

- Access-token expiry.
- Session storage redesign.
- Dependency upgrades, formatting sweeps, unrelated warnings.

## Requirements

- **R-001 (MUST):** Validate expiry before the rotation transaction begins.
- **R-002 (MUST):** Map `TokenError::Expired` to HTTP 401 with error code `refresh_token_expired`.
- **R-003 (MUST NOT):** Write any session state (row, counter, replacement token) on the expired path.
- **R-004 (MUST):** Implement the final design directly. No compatibility layer, dual path, or fallback. Remove `legacy_expiry_check`.

## Acceptance criteria

Typed acceptance uses the taskfmt Markdown profile: each `AC-*` block presents its fenced
Gherkin-shaped behavior first, then bulleted verification metadata and one fenced `sh` command.
It is not a Cucumber document and has no runtime step definitions.

### AC-001 — Expired refresh is rejected
```gherkin
Given an existing session and an expired refresh token
When the refresh endpoint receives the token
Then it returns 401 with error code refresh_token_expired
And no session state is changed
```

**Verification**

- **Type:** scenario
- **Covers:** `R-001, R-003`
- **Expected:** exit 0, `1 passed`; response is 401 and session state is unchanged

```sh
cargo test -p auth expired_refresh_token
```

### AC-002 — Valid refresh still rotates
```gherkin
Given a valid refresh token
When the refresh endpoint receives the token
Then rotation succeeds exactly as before
```

**Verification**

- **Type:** scenario
- **Covers:** `R-002`
- **Expected:** exit 0, `1 passed`; rotation succeeds as before

```sh
cargo test -p auth valid_refresh_rotation
```

### AC-003 — Legacy helper is absent
```gherkin
Given the repository after the change
When the source tree is inspected
Then the legacy expiry helper is absent
```

**Verification**

- **Type:** scenario
- **Covers:** `R-004`
- **Expected:** exit 0, no output; no reference remains

```sh
! grep -rn legacy_expiry_check src tests
```

### AC-004 — Completion gate passes
**Verification**

- **Type:** gate
- **Expected:** exit 0 and the last line is DONE

```sh
taskfmt verify
```

## Fixed decisions

- **D-001:** Expiry is checked via the existing `TokenStore::validate`; no new validator type.
- **D-002:** Error code string is exactly `refresh_token_expired` (see `docs/decisions/D-041.md`).
- **D-003:** No feature flag; the old ordering is deleted, not gated.

## Checklist

Static plan (grammar and state handling: see AGENTS.md). Each `AC-*` is cited on the item whose evidence is that AC's command. State lives in `progress.md`.

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-002` pass — evidence: both commands exit 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p auth expired_refresh_token` shows `1 failed` — `rotation_counter == 1, want 0`.
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Expiry validated before the transaction (`R-001`, `R-003`) — evidence: `cargo test -p auth expired_refresh_token` exits 0.
        - [ ] **2.1.1** `TokenStore::validate` called before `begin_transaction` in `rotate` — evidence: `grep -n "validate\|begin_transaction" src/auth/session/rotate.rs` shows validate on an earlier line.
        - [ ] **2.1.2** Counter increment moved inside the post-validation branch (`AC-001`) — evidence: `cargo test -p auth expired_refresh_token` → `1 passed` (the `rotation_counter == 0` assertion holds).
    - [ ] **2.2** Handler maps `TokenError::Expired` to 401 `refresh_token_expired` (`R-002`, `D-002`) — evidence: `cargo test -p auth expired_error_code` exits 0.
    - [ ] **2.3** `legacy_expiry_check` removed (`R-004`, `D-003`, `AC-003`) — evidence: `! grep -rn legacy_expiry_check src tests` exits 0.
    - [ ] **2.4** Valid refresh path unchanged (`AC-002`) — evidence: `cargo test -p auth valid_refresh_rotation` → `1 passed`.
- [ ] **3** Gate passes.
    - [ ] **3.1** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated — evidence: `git status --porcelain` and `git diff --no-renames --stat $TASKFMT_BASE` show only in-scope files.
    - [ ] **3.2** `taskfmt verify` exits 0 with last line `DONE` (`AC-004`) — evidence: final full run (with progress check); full output in the transcript.
<!-- checklist:end -->
