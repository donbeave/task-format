# Task examples

These seven `task/v5` packages are authentic, ordered implementation tasks for the `pgtui` Rust
workspace. They demonstrate real task contracts; they are not self-contained `taskfmt` smoke
fixtures. Each task keeps its own `verify.toml` and lists its prerequisites and writable scope.

## Order and prerequisites

Each task builds on the committed result of its predecessor. Select a task whose starting state and
prerequisites are available.

| Task | Slice | Depends on | Additional prerequisites |
| --- | --- | --- | --- |
| [TASK-001](TASK-001/README.md) | Bootstrap the workspace and stub binaries | Empty `pgtui` repository | Rust toolchain and task-specific starter inputs |
| [TASK-002](TASK-002/README.md) | Connection store, list, and CLI skeleton | TASK-001 | Rust toolchain and task-specific tests |
| [TASK-003](TASK-003/README.md) | Connection form and save flow | TASK-002 | Rust toolchain and task-specific tests |
| [TASK-004](TASK-004/README.md) | PostgreSQL connection and table browser | TASK-003 | Docker, database fixtures, and tests |
| [TASK-005](TASK-005/README.md) | Preview grid and client-side sorting | TASK-004 | Docker and grid/database tests |
| [TASK-006](TASK-006/README.md) | Custom SQL screen | TASK-005 | Docker and custom-SQL tests |
| [TASK-007](TASK-007/README.md) | Disconnect, exit behavior, and screen gallery | TASK-006 | Docker and gallery tests |

The individual task README and `verify.toml` define exact inputs, checks, writable paths, and
forbidden changes. Each package contains task-specific files under `trusted/`; these are
caller-provided starting inputs, not an automatic overlay. Paths beneath `trusted/` are relative to
the target checkout after removing that leading directory. For example,
`examples/TASK-001/trusted/crates/pgtui/src/render.rs` maps to
`$WORKSPACE/crates/pgtui/src/render.rs`. Materialize the selected task's inputs before recording
the baseline commit, then keep them unchanged while implementing the task.

TASK-004 through TASK-007 declare database integration checks that require Docker in the caller's
environment. Taskfmt does not create the target workspace, copy task inputs, install tools, or
provision services. The caller supplies those prerequisites.

## Caller workflow

The caller provides a Git checkout of the `pgtui` target repository, required input files and
services, a committed starting baseline, and a writable progress file outside the task package.
Declared commands run from the target repository's root. Prepare each task's target state before
implementation, commit that state, then record its full commit ID for `--base`. For TASK-002 through
TASK-007, begin from the committed result of the preceding task and prepare any additional inputs
required by the selected package.

Lint the task without a progress file:

```sh
TASK_DIR=/path/to/task-format/examples/TASK-001
WORKSPACE=/work/pgtui
TASK_ID=TASK-001
FIRST_LEAF=1.1
PROGRESS_FILE="${TMPDIR:-/tmp}/taskfmt-progress/${TASK_ID}.md"
BASE=$(git -C "$WORKSPACE" rev-parse --verify 'HEAD^{commit}')

taskfmt lint "$TASK_DIR"
```

From the task-format repository root, start progress from the canonical seed outside the task
directory. Replace its task ID and first leaf with the selected task's values:

```sh
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

While work remains, checks-only verification may run repeatedly. It validates the task, checks
workspace scope against the committed baseline, and runs declared commands without reading
progress or claiming completion. Some declared checks can fail before their implementation is
complete.

```sh
taskfmt verify --task-dir "$TASK_DIR" --root "$WORKSPACE" \
  --base "$BASE" --no-progress
taskfmt status --task-dir "$TASK_DIR" --progress "$PROGRESS_FILE"
```

After every checklist leaf is recorded complete, run full verification:

```sh
taskfmt verify --task-dir "$TASK_DIR" --root "$WORKSPACE" \
  --progress "$PROGRESS_FILE" --base "$BASE"
```

Progress `DONE` and `100%` describe the caller's checklist record only. Checks-only success ends
with `CHECKS PASS`. Only full verification success, with all declared checks and completed progress,
ends with the standalone line `DONE`.
