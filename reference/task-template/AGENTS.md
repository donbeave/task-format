# Task execution

`README.md` is the task contract. Keep the task package read-only; make changes in the caller's
workspace and within `verify.toml` path limits. The caller provides the workspace, tools, and inputs.

## Start

1. Read `README.md` and `verify.toml`; run `taskfmt lint TASK_DIR`.
2. Copy the file to the caller's chosen writable path with `cp "$TASK_DIR/progress.md" "$PROGRESS_FILE"`.
   Replace the placeholder task ID.
   Replace initial leaf `1.1` in both the `current` header and first event if the instantiated
   task's first leaf differs.
3. Begin from the first checklist leaf, unless the caller supplied valid progress to resume.

## Update progress

Append event rows in order. Sequence numbers start at 1 and stay contiguous. The header fields
`state`, `current`, and `latest_event` must match the event stream:

- `STARTED` opens an incomplete leaf when no leaf is active.
- `DONE` closes the active leaf. If work remains, immediately start the next leaf before saving.
  The derived state becomes `DONE` only after every leaf is done.
- `FAILED` closes the active leaf; append a next event before saving an in-progress file.
- `REOPENED` reopens a completed leaf as the active leaf.
- `BLOCKED` and `NEEDS_REPLAN` end the stream with the active leaf.

After edits, inspect state with `taskfmt status --task-dir TASK_DIR --progress PROGRESS_FILE`.
Write handoff notes as paragraphs or labels under `## Handoff`; do not start a line with `- ` or
use a line exactly equal to `## Events` or `---`.

## Verify

Use `taskfmt verify --task-dir TASK_DIR --root WORKSPACE --base BASE --no-progress` to run the
declared checks while progress is incomplete. Once every checklist leaf is complete, run full
verification with `--progress PROGRESS_FILE` and the caller's baseline. Full verification must
pass before reporting completion. Progress and 100% status record coordination; they do not prove
that declared checks passed.
