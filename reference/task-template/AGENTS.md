# Task execution

`README.md` is the task contract. Treat the task package as read-only; make implementation changes
in the caller-provided workspace and within `verify.toml` path limits. Before execution, the caller
must instantiate the task README and verifier and supply the workspace, baseline commit, tools,
services, and other prerequisites.

## Start

1. Read `README.md` and `verify.toml`; they must already contain the instantiated task ID, paths,
   check references, and real commands. Do not edit the task contract.
2. Use the caller-provided writable `PROGRESS_FILE` outside the task package. If starting new work
   without a progress file, set `TASK_FORMAT_ROOT` to the task-format checkout and copy the seed:

   ```sh
   TASK_FORMAT_ROOT=/path/to/task-format
   PROGRESS_FILE="${TMPDIR:-/tmp}/taskfmt-progress/progress.md"
   mkdir -p "$(dirname "$PROGRESS_FILE")"
   cp "$TASK_FORMAT_ROOT/reference/task-template/progress.md" "$PROGRESS_FILE"
   ```

   Set `task` to the task ID. Set `current` and the initial `STARTED` event's leaf to the first
   checklist leaf. Keep `state: IN_PROGRESS` and `latest_event: 1` for this initial event. There is
   no taskfmt initialization or progress-writing command. For resumed work, use the supplied valid
   progress file unchanged.
3. Run `taskfmt lint TASK_DIR` before editing the workspace. Start at the current checklist leaf,
   unless valid progress identifies a different leaf.

## Update progress

Append event rows in order. Sequence numbers start at 1 and stay contiguous. The event stream is the
source of truth; after each update, set `state`, `current`, and `latest_event` to the values derived
from the full stream.

- `STARTED` opens an incomplete leaf when no leaf is active.
- `DONE` closes and completes the active leaf. If work remains, immediately append `STARTED` for
  the next leaf. When every leaf is complete, set `state: DONE` and `current: NONE`.
- `FAILED` closes the active leaf without completing it. If work remains, append `STARTED` for the
  next leaf or a retry before saving.
- `REOPENED` opens a completed, inactive leaf again.
- `BLOCKED` and `NEEDS_REPLAN` end the stream with the active leaf still current.

The final event number is `latest_event`. `IN_PROGRESS` requires an active leaf. `DONE` requires a
`DONE` event for every checklist leaf not later reopened, with no active leaf. Do not add events
after `BLOCKED` or `NEEDS_REPLAN`.

Keep handoff notes under `## Handoff` as paragraphs or labels. Do not start a line with `- ` or use
a line exactly equal to `## Events` or `---` there.

Inspect progress with:

```sh
taskfmt status --task-dir TASK_DIR --progress PROGRESS_FILE
```

Status validates the task and progress and reports checklist completion and derived state. A `DONE`
progress state or `100%` status is a coordination claim, not proof that checks passed.

## Verify

Use checks-only verification while work remains:

```sh
taskfmt verify --task-dir TASK_DIR --root WORKSPACE --base BASE --no-progress
```

It validates the task, checks workspace scope against the supplied committed baseline, and runs
declared checks. It does not read progress and reports `CHECKS PASS` on success.

After all checklist leaves are complete, run full verification with the caller's progress file and
the same baseline:

```sh
taskfmt verify --task-dir TASK_DIR --root WORKSPACE \
  --progress PROGRESS_FILE --base BASE
```

Full verification must pass before reporting completion. It emits a final standalone `DONE` only
when the declared checks, scope rules, task validation, and completed progress all pass. A failed
check or invalid progress never emits that completion signal.
