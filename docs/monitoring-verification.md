# Monitoring acceptance evidence

## Project-root and CLI extension

Verified on 2026-09-07 after migrating the catalog to `projects/`:

- All 299 moved catalog files and 127 untouched legacy experiment files retain
  their exact bytes and modes. The catalog preserves five projects and 17 tasks.
- The default `task-monitor` scans `projects/`; its live API returns 17 tasks.
  The demo's seven `demo/pgtui` task packages lint with zero errors or warnings.
- Full Rust tests (including 302 library tests), strict all-target Clippy,
  formatting, release builds, installation, and `taskfmt selftest` pass.
  The new project/group CLI integration suite passes all 12 tests.
- All four Docker images rebuilt successfully; the enabled Docker proof passes.
  Host, live source, and all four image fingerprints match:
  `52c960db74b3b288ce93211c82e5703a338ba5ddfd40c92d6054ef93cfcd94e4`.
- The old run store had no executions or artifacts and was archived before
  creating the store bound to `projects/`. No paid agent tasks were launched.

## Original monitoring implementation

This record describes verification of the monitoring implementation on 2026-09-07,
before the catalog-root rename and first-class project/group CLI extension.
Counts and fingerprints below are historical evidence, not a verification claim
for subsequent changes.
Commands are reproducible in `monitoring.md`; generated build directories and
temporary execution fixtures are not committed.

Final gates passed: 302 Rust unit tests plus the existing integration/doctest
suites, taskfmt selftest, strict Clippy, formatting, 20 frontend tests, frontend
lint/typecheck/build, Rust release builds, and the enabled Docker proof runner.
All four taskfmt/base/Claude/Codex images were rebuilt. Host binary, live source,
and all four image binaries have the same source fingerprint:
`b3f207916991e1f99985b3bcadca3b8e1e3f24ba2d3dd5ad2f81e9423b9d0243`.

| Requirement | Evidence |
| --- | --- |
| Preserve flat taskfmt contracts and commands | Existing Rust corpus, resume, run-resolution, fingerprint, consent, gate-tamper, selfcheck, and lifecycle suites; `taskfmt selftest`. Source resolution has separate location and contract identity tests. |
| Canonical filesystem discovery and strict metadata | `harness/src/monitor/tests.rs`: hierarchy, normalized IDs, duplicate/missing/self/cyclic dependencies, strict verifier and metadata validation. |
| Representative real task packages | Initial four projects and four groups under the former `tasks/` root (now `projects/`); ten unchanged original task contracts and trusted assets. All ten packages linted clean. Initial totals: one draft, nine pending, six blocked, three ready, zero done. Demo subsequently added seven packages; see `monitoring-demo.md`. |
| Domain-owned status, readiness, progress and aggregation | Domain tests cover draft exclusion, transitions, external dependencies, scope validation, aggregate arithmetic, partial progress, reopened leaves, failure and latest event. |
| Root confinement and durable ownership | Domain path-traversal, symlink, atomic-write and OS-lock tests; server tests for invalid IDs, foreign origins, form submissions, rebinding, overlapping roots, dual locks and durable catalog binding. |
| Verified completion only | Real integration fixture invokes taskfmt's immutable host gate and checks actual matcher evidence; forged done and mismatched contract evidence are rejected. |
| Task/group/project API | `harness/src/server/tests.rs` and `integration_tests.rs`: typed success/error responses, eligibility, conflicting starts, malformed catalogs and all run scopes. |
| Dependency scheduling and concurrency | Real adapter integration tests prove diamond ordering, independent overlap at concurrency two, serialization at concurrency one, and no dependent launch after failed verification. |
| Restart and process cleanup | Stale-state recovery tests and real supervisor subprocess tests for parent SIGKILL and terminal process-group SIGINT. |
| Dashboard, project, Kanban, detail, My Tasks | Twenty Vitest tests exercise actual nested routes, aggregates, empty states, status/readiness, API validation/actions, stale refresh, safe Markdown and DAG lanes. |
| Responsive and keyboard access | Real Chrome checks at 1440px and 320px across all five routes; skip link, mobile navigation, Kanban and code scrolling, API failure and Retry recovery. |
| Basic accessibility | Axe WCAG A/AA scans across the ten route/viewport combinations reported no remaining violations after fixing checklist labels and keyboard scroll regions. Screenshots also exposed and verified the fix for clipped mobile contract content. |
| Production frontend | Bun frozen install, Biome lint, strict TypeScript, Vitest and Vite production build. Production preview serves all nested paths and proxies real catalog data. |
| Current dependencies and provenance | `monitoring-versions.md`, Cargo.lock and bun.lock; one Base UI primitive stack, no Radix or alternative package-manager locks. Source-owned shadcn notices and clean reference-concept provenance are recorded. |
| Docker compatibility | Actual taskfmt/base image builds with Rust 1.98.1; enabled Docker integration proves prerequisite lifecycle and platform checks, not merely a skipped Cargo test target. |

The deterministic integration executable replaces taskfmt's dispatch and coding
agent portion: it prepares a temporary workspace, snapshot, progress and manifest,
then invokes the actual immutable taskfmt host gate.
It does not replace HTTP scheduling, the production process adapter and supervisor,
filesystem status transitions, gate evaluation, or authoritative completion checks.
Separate existing CLI, lifecycle and enabled Docker suites verify their respective
dispatch, promotion and container prerequisite boundaries.
The seeded pgtui contracts were not sent to a paid agent during verification.
Their predecessor provenance still requires the existing operator repository
preparation/promotion workflow; the monitor does not silently publish code.

Reference applications were inspected for shell, routes, navigation, components,
loading, responsive behavior, task boards, and My Tasks concepts by separate
agents. Separate domain, API, frontend, compatibility, stack-research, browser,
and final-review workstreams reported findings back to integration.
No proprietary reference source or demo assets were copied.
