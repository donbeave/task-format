---
schema: task/v5
id: TASK-001
title: "Bootstrap the pgtui workspace from an empty main"
kind: feature
---

# TASK-001 — Bootstrap the pgtui workspace from an empty main

## Goal

Create the pinned Rust workspace. Both binaries remain exit-2 stubs until later slices replace them.

## Context

The trusted base supplies the render module, font, and behavioral tests. Build the single `pgtui` member with its library, application binary, and gallery binary; expose only the trusted render module.

## Preconditions

- **P-001:** Required Rust toolchain is available.
- **P-002:** Trusted scaffold material is present and unchanged.

## Scope

In scope: workspace bootstrap, pins, toolchain metadata, member manifest, library surface, and stubs. Out of scope: application features, trusted material, documentation, CI, and theming.

## Requirements

- **R-001:** Create the D-001 single-member 2024 workspace with exact pins and committed lockfile.
- **R-002:** Declare the library and both binaries; member dependencies use workspace inheritance.
- **R-003:** Configure the D-001 toolchain, formatter, ignore list, and container setting.
- **R-004:** Export exactly the trusted render module from the library.
- **R-005:** Keep the D-040 `pgtui` and `gallery` exit-2 stub contracts.
- **R-006:** Implement directly, without compatibility paths or feature flags.
- **R-007:** Do not alter trusted material, unapproved dependencies, or unrelated artifacts.

## Acceptance criteria

### AC-001 — Workspace builds
```gherkin
Given the trusted scaffold
When the workspace is built
Then every target compiles
```
**Verification**
- **Type:** scenario
- **Covers:** R-001
- **Check:** CHK-001

### AC-002 — Pins and metadata are exact
```gherkin
Given the bootstrap configuration
When its pins and metadata are inspected
Then they match D-001
```
**Verification**
- **Type:** invariant
- **Covers:** R-002, R-003
- **Check:** CHK-002

### AC-003 — Stub and render behavior holds
```gherkin
Given the bootstrap binaries and library
When trusted behavioral tests run
Then both stub contracts and the render surface hold
```
**Verification**
- **Type:** scenario
- **Covers:** R-004, R-005
- **Check:** CHK-003

### AC-004 — Scope remains protected
```gherkin
Given the candidate workspace
When protected-source rules are evaluated
Then trusted material and unrelated artifacts remain untouched
```
**Verification**
- **Type:** invariant
- **Covers:** R-006, R-007
- **Check:** CHK-004

### AC-005 — Package gate passes
**Verification**
- **Type:** gate
- **Check:** CHK-005

## Fixed decisions

- **D-001:** Use one Cargo 2024/resolver-3 workspace with only `crates/pgtui` (library plus `pgtui` and `gallery` binaries), committed lockfile, `unsafe_code = "forbid"`, and the exact pinned dependency set checked by CHK-002. Member dependencies inherit from the workspace; do not add or change dependencies.
- **D-002:** The library exports only planner-supplied `render`; `render.rs` and fonts are protected input, never executor output.
- **D-040:** Until their owning slices replace them, both binaries print `error: not implemented` to stderr and exit 2.
- **D-070:** Planner-supplied render/font/tests are read-only and outside writable scope; behavior tests assert semantics, not golden snapshots.

## Checklist

<!-- checklist:start -->
- [ ] **1** Bootstrap workspace.
    - [ ] **1.1** Establish the workspace and pins for R-001; prove AC-001 via CHK-001.
    - [ ] **1.2** Configure member and toolchain for R-002 and R-003; prove AC-002 via CHK-002.
- [ ] **2** Preserve bootstrap behavior.
    - [ ] **2.1** Keep render and stub contracts for R-004 and R-005; prove AC-003 via CHK-003.
    - [ ] **2.2** Keep scope protected for R-006 and R-007; prove AC-004 via CHK-004.
- [ ] **3** Complete package verification.
    - [ ] **3.1** Complete the package gate for R-006 and R-007; prove AC-005 via CHK-005.
<!-- checklist:end -->
