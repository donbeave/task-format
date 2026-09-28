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
`writable_paths`, and one or more `[[checks]]`. Replace the template `writable_paths` with the exact
relative files and directories the task may modify; it is the task's scope allowlist. Optional scope
constraints are `base_tree`, `forbidden_paths`, and `forbidden_patterns`. If `base_tree` is set, the
caller's `--base` must resolve to that same commit.

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

For example, from the task-format repository root, create an external progress file for task
`TASK-042` whose first checklist leaf is `1.1`:

```sh
TASK_ID=TASK-042
FIRST_LEAF=1.1
PROGRESS_FILE="${TMPDIR:-/tmp}/taskfmt-progress/${TASK_ID}.md"
mkdir -p "$(dirname "$PROGRESS_FILE")"
cp reference/task-template/progress.md "$PROGRESS_FILE"
PROGRESS_TMP="${PROGRESS_FILE}.tmp"
sed \
  -e "s/^task: TASK-000$/task: ${TASK_ID}/" \
  -e "s/^current: 1\\.1$/current: ${FIRST_LEAF}/" \
  -e "s/^- 1 | STARTED | 1\\.1$/- 1 | STARTED | ${FIRST_LEAF}/" \
  "$PROGRESS_FILE" > "$PROGRESS_TMP"
mv "$PROGRESS_TMP" "$PROGRESS_FILE"
```

The commands replace the task ID and initial leaf while keeping `state: IN_PROGRESS` and
`latest_event: 1` for the initial event. Keep the progress file outside the task package.

Allowed statuses are `STARTED`, `DONE`, `FAILED`, `REOPENED`, `BLOCKED`, and `NEEDS_REPLAN`:

- `STARTED` opens the globally earliest incomplete leaf when none is active, except immediately
  after `FAILED`, when it may retry that failed leaf or advance to its immediate ordered successor
  only if that successor is incomplete. If the successor is already complete or the failed leaf is
  last, retry the failed leaf.
- `DONE` closes the active leaf and records it complete.
- `FAILED` closes the active leaf without completing it. It may be retried; the next incomplete
  immediate successor may be started instead. After that successor is completed, the next `STARTED`
  returns to the globally earliest incomplete leaf. On the final checklist leaf, there is no
  successor, so retry it; a valid `REOPENED` event for a completed inactive leaf is also allowed.
- `REOPENED` opens any previously completed, inactive leaf again whenever no leaf is active,
  including immediately after `FAILED` and while other leaves remain incomplete. After the reopened
  leaf is completed with `DONE`, the next `STARTED` selects the globally earliest incomplete leaf.
- `BLOCKED` and `NEEDS_REPLAN` end the event stream and leave the active leaf current.

The `progress/v1` schema and event serialization are unchanged, but validation now enforces these
ordered `STARTED` rules. Only previously accepted out-of-order event streams may now fail
validation. Correct historical events only when they are factually wrong, and preserve the original
log. If the history is factually accurate but out of order, do not mutate or reorder it: preserve or
archive the old log, then start a replacement ordered progress stream based on actual workspace
state, beginning with the earliest unresolved leaf.

Keep `state`, `current`, and `latest_event` synchronized with the complete event stream:

| Derived state | `current` | Condition |
| --- | --- | --- |
| `IN_PROGRESS` | Active leaf ID | Work remains and a leaf is active. |
| `BLOCKED` | Active leaf ID | The final event is `BLOCKED`. |
| `NEEDS_REPLAN` | Active leaf ID | The final event is `NEEDS_REPLAN`. |
| `DONE` | `NONE` | Every leaf has a `DONE` event not later reopened. |

At the beginning and after `DONE`, if work remains, append `STARTED` for the globally earliest
incomplete leaf. After `FAILED`, the next event may be `REOPENED` for a completed inactive leaf;
otherwise append `STARTED` for the failed leaf, or its immediate ordered successor only when that
successor is incomplete. If the successor is already complete or the failed leaf is last, retry the
failed leaf. On the final checklist leaf, no successor exists, so retry it; a valid `REOPENED` event
for a completed inactive leaf is also allowed. After a started successor or reopened leaf is
completed with `DONE`, resume `STARTED` events at the globally earliest incomplete leaf. An
`IN_PROGRESS` file must have an active leaf.
`latest_event` equals the last event's sequence. The header's task ID matches the task README.

Write optional handoff notes as paragraphs or labels. Do not begin a handoff line with `- ` or use a
line exactly equal to `## Events` or `---`; those are reserved for the event structure.

`taskfmt status --task-dir TASK_DIR --progress PROGRESS_FILE` validates the task and progress, then
reports event-derived state, current item, per-item status, completed leaves, total leaves, and
percentage. Percentage is `100 × completed leaves / total leaves`, rounded to the nearest whole
number with halves rounded up. A task with no leaf is invalid. Status only reads files. Progress
state and `100%` are coordination claims; they do not prove that verification checks passed.

## Workflow

1. Copy `README.md`, `AGENTS.md`, and `verify.toml` from `task-template/` into a task directory.
   Replace placeholders, replace `writable_paths` with the task's actual allowed paths, and make the
   task contract and checks agree. Keep `progress.md` outside the task package.
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
