# Task Format

This document specifies the formats consumed by `taskfmt`: `task/v5`, `verify/v2`, and
`progress/v1`. A copyable instance is in [`task-template/`](task-template/).

## Task package: `task/v5`

A task directory contains `README.md` and `verify.toml`; `AGENTS.md` may give concise executor
instructions. The caller keeps progress at a separate writable path.

The task README starts with YAML-like frontmatter containing exactly these keys:

```yaml
---
schema: task/v5
id: TASK-042
title: "A concrete task title"
kind: feature
---
```

`id` is `TASK-` followed by digits. `kind` is one of `bugfix`, `feature`, `refactor`, `removal`,
`migration`, `test`, or `docs`. The first H1 starts with the same task ID. Include these H2
sections: `Goal`, `Context`, `Preconditions`, `Scope`, `Requirements`, `Acceptance criteria`,
`Fixed decisions`, and `Checklist`.

Requirements use unique `R-NNN` IDs. Acceptance criteria use unique `AC-NNN` headings and a
`**Verification**` block with `Type`, `Covers`, and `Check` fields. Types are `scenario`, `outline`,
`invariant`, and `gate`. Each non-gate criterion has a fenced `gherkin` behavior block and covers
one or more requirements. Scenarios and outlines have one `When` step and at least one `Then` step.
A gate has a `Check` and no `Covers` or behavior block. Each `Check` names a `CHK-NNN` in
`verify.toml`.

The checklist is enclosed by one `<!-- checklist:start -->` / `<!-- checklist:end -->` marker pair.
Items use numeric dotted IDs such as `1`, `1.1`, and `1.2`, with four spaces per nesting level.
An item is a leaf when the next item is not deeper. Every leaf links to at least one known `R-*`,
`AC-*`, and `CHK-*`; parent items group leaves. IDs are unique within the checklist. Progress is
derived from leaves only.

## Verification: `verify/v2`

`verify.toml` has `schema = "verify/v2"`, a `task_id` matching the README ID, non-empty relative
`writable_paths`, and one or more `[[checks]]`. Optional scope constraints include `base_tree`,
`forbidden_paths`, and `forbidden_patterns`. If `base_tree` is set, `taskfmt verify` requires the
caller-supplied `--base` to resolve to that same commit.

Each check has a unique `CHK-NNN` ID, a phase (`precondition`, `focused`, `regression`, `lint`, or
`gate`), and exactly one command form: `argv` or `shell`. It declares non-empty `requirements` and
`acceptance` references and may declare expected exit status, output matchers, or required and
forbidden artifacts. Checks must appear in phase order: `precondition`, `focused`, `regression`,
`lint`, then `gate`; `gate` is last. Exactly one check has phase `gate`. AC-to-check, check-to-AC,
and all task ID references must agree.

`taskfmt lint TASK_DIR` checks the README, TOML, and their references; it does not execute commands.
`taskfmt verify` executes declared commands in the caller's workspace, checks their expectations,
and enforces the supplied baseline and path limits. The caller provides all needed tools and inputs.
Arbitrary declared commands are not sandboxed by `taskfmt`.

## Progress: `progress/v1`

Progress is a caller-written Markdown file. Copy [`task-template/progress.md`](task-template/progress.md)
to the caller's chosen writable path, then substitute the task ID and the first checklist leaf in
both `current` and the initial event. `taskfmt` reads and validates this file; it does not create or
update it.

The opening header has exactly five fields: `schema`, `task`, `state`, `current`, and `latest_event`.
The body has an `## Events` section followed by `## Handoff`. An event row is exactly
`- N | STATUS | LEAF`, where `N` starts at 1 and increases contiguously. Rows refer only to checklist
leaves.

Allowed event statuses are `STARTED`, `DONE`, `FAILED`, `REOPENED`, `BLOCKED`, and `NEEDS_REPLAN`.
`STARTED` opens an incomplete leaf when none is active. `DONE` and `FAILED` close the active leaf.
If work remains after either event, append `STARTED` for the next leaf before saving the file;
otherwise the derived in-progress state would have no current leaf. `REOPENED` opens a previously
completed leaf again. `BLOCKED` and `NEEDS_REPLAN` end the event stream with the active leaf.

The header is derived from the complete event stream and must stay synchronized:

| Derived state | `current` | Condition |
| --- | --- | --- |
| `IN_PROGRESS` | Active leaf ID | Work remains and a leaf is active. |
| `BLOCKED` | Active leaf ID | The final event is `BLOCKED`. |
| `NEEDS_REPLAN` | Active leaf ID | The final event is `NEEDS_REPLAN`. |
| `DONE` | `NONE` | Every checklist leaf has a `DONE` event not later reopened. |

`latest_event` equals the final event sequence. The header's task ID must match the task README.
Use paragraphs or labels for handoff notes. The parser rejects any handoff line beginning with
`- ` and lines exactly equal to `## Events` or `---`. The template shows a valid initial stream for
`TASK-000` and leaf `1.1`.

`taskfmt status --task-dir TASK_DIR --progress PROGRESS_FILE` validates progress and reports its
derived checklist state. Its percentage is `100 × completed leaves / total leaves`, rounded to the
nearest integer with halves rounded up. Progress and status are coordination data; they do not prove
that verification commands passed.

## Verification workflow

1. Instantiate the task package from `task-template/README.md`, `AGENTS.md`, and `verify.toml`;
   leave `task-template/progress.md` outside the task directory.
2. Copy the progress seed to the caller's writable path and update it for the task.
3. Run `taskfmt lint TASK_DIR`.
4. Update progress events as work proceeds; inspect them with `taskfmt status`.
5. Run `taskfmt verify --task-dir TASK_DIR --root WORKSPACE --base BASE --no-progress` for
   checks-only verification while work remains.
6. Once all leaves are complete, run full verification with `--progress PROGRESS_FILE` and the
   caller's baseline. Only a successful full run, ending in `DONE`, confirms declared checks and
   completed progress together.
