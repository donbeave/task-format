# Task monitor design — Verve v1.1.0

Status: draft. User approval pending; no screen blessed or screenshot baseline frozen.
Reference revision: `0abdca346a8ab30014be459d206cfe4999224573`.
Reference concept: repository `extra/` Verve v1.1.0; application tokens own the implementation.
Synthetic fixtures only. Execution eligibility is false at every scope; dependency readiness includes one ready task to exercise the ready-task table without enabling execution.

## Shared matrix

- States: default (synthetic catalog), empty (no children; task identity absent), loading (shared catalog skeleton), error (shared readable catalog failure). Explicit route state wins; no live backend is queried.
- Themes: light and dark, controlled by the shipping shell.
- Viewports: 1440 × 900 desktop; 390 × 844 mobile; 320 × 844 narrow mobile.
- Responsive rules: shell navigation collapses; card grids stack; tables scroll inside their own container. No page-wide horizontal scroll intended.
- Components: shipping `components/views.tsx` screen exports and `components/shell.tsx` loading/error/frame, installed Button and Progress. Gallery index is semantic navigation, not a duplicate component renderer.
- Copy/fixtures: `fixtures/catalog.ts`; long task title and Fjörður / 日本語 / Nguyễn strings exercise wrapping.
- Component hash: `components/views.tsx` SHA-256 `0b3bab653019d5da403cb93e29f8c50188d5fcf16105b899a0312640ef8292b3`; `components/shell.tsx` SHA-256 `9de791c896808372ab53ff8fce3de9039c3966e4143e6779d394febf570a9bf6`; `components/monitor.tsx` SHA-256 `31019661ad78ff7c520de9489d1658691b787207229fe03940bcb578f72fb989`. Draft implementation snapshot, not an approval binding.
- Stylesheet hash: `styles.css` SHA-256 `9b848b85a6a7b41dd4e4c664cb22e29b12daa8f9f720f29cfa613d24e3480587`.
- Fixture hash: `fixtures/catalog.ts` SHA-256 `82833756a1fc3a3146eec1d2161a6d9f616c1d1fc61b10b9d47a14c32b21be5b`.
- Registry hash: `registry.tsx` SHA-256 `25c44e33f2485814281aaded57357c838df4ee66146a2ca0189fb3eb5104c5e8`.
- Blessed matrix: None — user review required for all 5 × 4 × 2 × 3 combinations.
- Blessed: None — no user approval received.
- Captures: None — not authorized in design phase.

## Overview

Purpose: summarize filesystem projects and task progress.
Route: `/design/dashboard/$state`; ships at `/` using `DashboardScreen`.
States, viewports, components, copy, revision, hashes and blessing: shared matrix above.

## Project

Purpose: review project groups and project-wide execution eligibility.
Route: `/design/project/$state`; ships at `/projects/$projectCode` using `ProjectScreen`.
States, viewports, components, copy, revision, hashes and blessing: shared matrix above.

## Group

Purpose: review task sequence, progress and dependency readiness.
Route: `/design/group/$state`; ships at `/projects/$projectCode/groups/$groupCode` using `GroupScreen`.
States, viewports, components, copy, revision, hashes and blessing: shared matrix above.

## Task

Purpose: inspect contract, verification progress and dependency context.
Route: `/design/task/$state`; ships at `/projects/$projectCode/groups/$groupCode/tasks/$taskNumber` using `TaskScreen`.
Empty is an unavailable task identity, not a fabricated task with no contract.
States, viewports, components, copy, revision, hashes and blessing: shared matrix above.

## My tasks

Purpose: inspect dependency-aware task readiness across the workspace.
Route: `/design/my-tasks/$state`; ships at `/tasks/my` using `MyTasksScreen`.
States, viewports, components, copy, revision, hashes and blessing: shared matrix above.

## Guard and review

Routes exist only in development or when `VITE_DESIGN_ROUTES=1`; production defaults to not found.
Gallery uses the shipping pure components; it is not a mockup or a second CSS renderer.
Fixture actions are disabled; error retry is an intentional no-op in this deterministic preview.
Approval must bind final component, fixture, registry hashes and this complete matrix before baselines are frozen.

## Executable checks

Full frontend gate: 51 tests, strict typecheck, Biome lint/format, and production
build pass. No backend source changed.

`test/design.test.tsx`: 28 passing tests cover every screen/state, valid synthetic catalog schemas, disabled execution, independent readiness, backend-offline rendering without GET/POST, unknown identifiers returning not found, and the actual route guard with production environment values both disabled and explicitly enabled. These environment-stubbed tests prove guard behavior, not production bundle removal. No browser visual approval or screenshot baseline is implied.

Live browser checks on 2026-09-07 exercised the five real routes and 20 preview
states at the three listed viewports in both themes. Fixed the 320px header
overflow and verified that the named ready-task table scrolls with arrow keys.
Real network failure and loading states retain their layout at 320px. Ten Axe
WCAG A/AA checks (five live routes, both themes at 320px, including contrast)
reported zero violations. No screenshots were captured.

## Ownership and recovery

Allowed writes are the frontend shell/screens, their stylesheet and tests,
guarded design routes/fixtures/manifest, presentation context/root integration,
frontend dependency pins/lockfile, generated route registry, and frontend README.
No backend rules, task packages, execution evidence, or reference source changed.
Existing Button/Progress are reused unchanged. Semantic details/table elements
retain browser disclosure/table behavior; the existing compound frame/card markup
is retained because a single Card does not express the reference's inset frame
and footer hierarchy. No additional component framework was introduced.

Reference dimensions (63px rail, 50px header, 200px sidebar) intentionally use
the source's exact measurements rather than rounding them to the spacing scale.
Neutral token values follow the reference; muted text is slightly darker in light
mode to maintain contrast at the compact text sizes. Fonts are self-hosted.

Work began at the bound revision above with only unrelated `TASK-MONITORING-GOAL.md`
and `extra/` untracked; both are preserved. Temporary staging/recovery directories:
`/tmp/task-monitor-verve.HA9oln`, `/tmp/task-monitor-primitives.9qNUj4`,
`/tmp/verve-screens.h8yTMA`, and `/tmp/verve-gallery.fxO2xr`.
Approval, commit, push, and screenshot-baseline freezing are not performed by
this draft handoff. Backend suites were not rerun: backend source is unchanged.
