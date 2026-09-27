# Task format reference

This reference specifies the three formats read by `taskfmt`: `task/v5`, `verify/v2`, and
`progress/v1`. [`task-template/`](task-template/) contains a copyable seed; replace its placeholders
before using it as a real task.

## Task contract: `task/v5`

A task package contains `README.md` and `verify.toml`. `AGENTS.md` may contain concise instructions
for the executor. The caller keeps mutable progress at a separate writable path.

The task README begins with frontmatter containing exactly these keys:

```yaml
---
schema: task/v5
id: TASK-042
title: "A concrete task title"
kind: feature
---
```

`id` is `TASK-` followed by digits. `kind` is one of `bugfix`, `feature`, `refactor`, `removal`,
`migration`, `test`, or `docs`. The first H1 begins with the same ID. Include these H2 sections:
`Goal`, `Context`, `Preconditions`, `Scope`, `Requirements`, `Acceptance criteria`, `Fixed
decisions`, and `Checklist`.

Requirements use unique `R-NNN` IDs. Acceptance criteria use unique `AC-NNN` headings and a
`**Verification**` block with `Type`, `Covers`, and `Check` fields. Types are `scenario`, `outline`,
`invariant`, and `gate`. Every non-gate criterion has a fenced `gherkin` behavior block and covers
one or more requirements. Scenarios and outlines have one `When` step and at least one `Then` step.
A gate has a `Check` and no `Covers` or behavior block. Each `Check` names a `CHK-NNN` in
`verify.toml`.

The checklist is enclosed by exactly one `<!-- checklist:start -->` / `<!-- checklist:end -->` marker
pair. Items use unique numeric dotted IDs such as `1`, `1.1`, and `1.2`, with four spaces per
nesting level. An item is a leaf when the next item is not deeper. Every leaf links to at least one
known `R-*`, `AC-*`, and `CHK-*`; parents group leaves. Progress counts leaves only. A task needs at
least one checklist leaf.

## Local checks: `verify/v2`

`verify.toml` has `schema = "verify/v2"`, a `task_id` matching the README ID, non-empty relative
`writable_paths`, and one or more `[[checks]]`. Optional scope constraints are `base_tree`,
`forbidden_paths`, and `forbidden_patterns`. If `base_tree` is set, the caller's `--base` must
resolve to that same commit.

Each check has a unique `CHK-NNN` ID and phase (`precondition`, `focused`, `regression`, `lint`, or
`gate`). Checks appear in that order; exactly one `gate` check is last. A check has exactly one
command form: `argv` or `shell`. It declares non-empty `requirements` and `acceptance` references
that agree with the task. Expected results may constrain exit status, stdout and stderr text or
regular expressions, occurrence counts, and required or forbidden artifacts.

`taskfmt lint TASK_DIR` validates the task README, TOML, and cross-references without executing
commands or reading progress. `taskfmt verify` runs the declared checks in the supplied workspace,
checks their expected results, and enforces the supplied baseline and path limits. `argv` runs the
named executable directly. `shell` runs through Bash with error and pipeline-failure handling. The
caller supplies required tools, services, trusted inputs, and a Git workspace with a committed
baseline. Taskfmt never substitutes a different baseline when `--base` is missing or invalid.

Declared commands can have side effects. Taskfmt does not sandbox them or provide containers,
credentials, services, agents, dispatch, or execution orchestration.

## Caller progress: `progress/v1`

Progress is a caller-written Markdown file outside the task package. Copy
[`task-template/progress.md`](task-template/progress.md) to a writable location, then replace its
task ID and initial leaf with the ID and first leaf from the task. Taskfmt reads and validates
progress; it does not create or update the file.

The opening header has exactly five fields: `schema`, `task`, `state`, `current`, and
`latest_event`. The body contains an `## Events` section followed by `## Handoff`. Each event row is
exactly `- N | STATUS | LEAF`; sequence numbers start at 1 and increase contiguously. Events refer
only to checklist leaves. Use one blank line after the header fence and one blank line between the
last event row and `## Handoff`.

For example, from the task-format repository root, create a progress file for task `TASK-042` whose
first checklist leaf is `2.1`:

```sh
PROGRESS_FILE="${TMPDIR:-/tmp}/taskfmt-progress/TASK-042.md"
mkdir -p "$(dirname "$PROGRESS_FILE")"
cp reference/task-template/progress.md "$PROGRESS_FILE"
```

In the copied file, replace `task: TASK-000` with `task: TASK-042`, replace `current: 1.1` with
`current: 2.1`, and replace `- 1 | STARTED | 1.1` with `- 1 | STARTED | 2.1`. Keep
`state: IN_PROGRESS` and `latest_event: 1` for this initial event.

Allowed statuses are `STARTED`, `DONE`, `FAILED`, `REOPENED`, `BLOCKED`, and `NEEDS_REPLAN`:

- `STARTED` opens an incomplete, not-yet-completed leaf when none is active.
- `DONE` closes the active leaf and records it complete.
- `FAILED` closes the active leaf without completing it. Start the next leaf or retry before saving
  an in-progress file.
- `REOPENED` opens a previously completed, inactive leaf again.
- `BLOCKED` and `NEEDS_REPLAN` end the event stream and leave the active leaf current.

Keep `state`, `current`, and `latest_event` synchronized with the complete event stream:

| Derived state | `current` | Condition |
| --- | --- | --- |
| `IN_PROGRESS` | Active leaf ID | Work remains and a leaf is active. |
| `BLOCKED` | Active leaf ID | The final event is `BLOCKED`. |
| `NEEDS_REPLAN` | Active leaf ID | The final event is `NEEDS_REPLAN`. |
| `DONE` | `NONE` | Every leaf has a `DONE` event not later reopened. |

After `DONE` or `FAILED`, if work remains, append `STARTED` for the next or retried leaf before
saving. An `IN_PROGRESS` file must have an active leaf. `latest_event` equals the last event's
sequence. The header's task ID matches the task README.

Write optional handoff notes as paragraphs or labels. Do not begin a handoff line with `- ` or use a
line exactly equal to `## Events` or `---`; those are reserved for the event structure.

`taskfmt status --task-dir TASK_DIR --progress PROGRESS_FILE` validates the task and progress, then
reports event-derived state, current item, per-item status, completed leaves, total leaves, and
percentage. Percentage is `100 × completed leaves / total leaves`, rounded to the nearest whole
number with halves rounded up. A task with no leaf is invalid. Status only reads files. Progress
state and `100%` are coordination claims; they do not prove that verification checks passed.

## Workflow

1. Copy `README.md`, `AGENTS.md`, and `verify.toml` from `task-template/` into a task directory.
   Replace placeholders and make the task contract and checks agree. Keep `progress.md` outside the
   task package.
2. Copy `task-template/progress.md` to the caller's writable progress path. Replace `TASK-000` and
   `1.1` with the task ID and its first checklist leaf.
3. Prepare the caller's workspace and prerequisites. Commit its starting state and record the full
   commit ID for `--base`.
4. Run `taskfmt lint TASK_DIR`.
5. Record progress events while working. Inspect derived status with
   `taskfmt status --task-dir TASK_DIR --progress PROGRESS_FILE`.
6. When useful, run checks-only verification:

   ```sh
   taskfmt verify --task-dir TASK_DIR --root WORKSPACE --base BASE --no-progress
   ```

   This runs task validation, scope checks, and declared commands without reading progress. A
   successful checks-only run ends with `CHECKS PASS`, not `DONE`.

7. After every checklist leaf is recorded complete, run full verification:

   ```sh
   taskfmt verify --task-dir TASK_DIR --root WORKSPACE \
     --progress PROGRESS_FILE --base BASE
   ```

   Only successful full verification checks both the declared commands and completed progress. It
   ends with the standalone line `DONE`. A `DONE` progress state or `100%` status alone never
   produces that completion signal.
