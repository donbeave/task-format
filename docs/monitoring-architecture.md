# Filesystem task monitoring architecture

## Existing invariants

The existing harness owns task execution and authoritative verification.
Its strict `task/v5` document, `verify/v2` configuration, flat `TASK-*` packages,
and external `progress/v1` records remain valid.
Checklist progress is an agent report, not completion proof.
Only a successful host gate for the recorded candidate can authorize completion.

Hierarchical catalog identity and task contract identity are separate concepts.
`jackin/new-design/001` identifies a package in the monitoring catalog; the
unchanged document inside that package can identify its contract as `TASK-001`.
Execution resolves the catalog identifier to a validated package and uses a safe
run identifier instead of interpolating a hierarchical path into container names.

## Boundaries

`harness/src/monitor/` owns catalog discovery, metadata, dependency validation,
status transitions, eligibility, aggregate progress, and durable filesystem state.
`harness/src/server/` adapts that domain to HTTP and existing taskfmt execution.
The `task-monitor` executable binds locally and accepts roots and execution
configuration from the operator, never from browser requests.
`web/` contains the Bun-managed TanStack Start application.

The filesystem is the database, rooted at `projects/` by default and selected
with `--projects-root`. Project and group README files provide names and
descriptions. Task README and verification files remain immutable contracts.
The versioned `task.toml` sidecar holds mutable lifecycle status and canonical
dependency references. Run artifacts remain separate from task contracts.
Project and group counts are computed from task records, never persisted.

## Lifecycle and scheduler

Draft tasks remain visible but do not execute or block readiness.
Pending tasks become eligible when all non-draft dependencies are done.
Blocked is a derived property of pending tasks, not a persisted fifth status.
An execution must acquire ownership before transitioning a task to in progress.
Successful authoritative host verification permits done; failure returns pending
with run evidence available to explain the result.
Restart reconciliation uses durable run state and ownership, not merely elapsed time.

Task, group, and project execution use the same domain eligibility rules.
Group and project runs require all executable tasks to start pending and no
conflicting active execution. Scheduling follows the dependency DAG with bounded
parallelism. A failed prerequisite prevents its dependents from starting.
External dependencies remain visible and must satisfy readiness; execution of a
scope must never silently expand to another scope.

## HTTP and browser

The API exposes the validated catalog, derived eligibility, execution state, and
task/group/project run actions. Errors use typed JSON and appropriate HTTP status.
Browser input selects catalog IDs; it cannot supply filesystem paths or commands.
Markdown is sanitized at the rendering boundary.
Frontend routes cover dashboard, project, group Kanban, task detail, and My Tasks.
Polling refreshes progress and execution without introducing a WebSocket service.

## Reference concepts

Verve supplies the visual vocabulary: compact navigation rail and contextual
sidebar, inset bordered frames, neutral surfaces, and compact Kanban cards.
Tempo supplies Start route structure and readable grouped task lanes.
Tempo's reference My Tasks view is a Gantt tree, not a dependency DAG; this
application must compute actual topological lanes and explicit dependency links.
Both references carry proprietary licenses. No reference source or demo assets
are copied; concepts are independently implemented.
See `monitoring-references.md` for inspected reference paths and adopted concepts.

## Verification boundary

Completion requires preserved harness tests, domain and API safety tests,
temporary-filesystem execution integration, frontend route/action/accessibility
tests, formatting, lint, typecheck, and production builds.
The enabled Docker integration runner must prove its test bodies executed;
a normal Rust test run that reports Docker skipped is insufficient evidence.
Implementation and operator documentation will record exact commands and schemas.
