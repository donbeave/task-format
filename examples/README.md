# Task examples

These seven task/v5 packages are real, ordered implementation slices for the
`pgtui` Rust workspace. They are target-repository examples, not self-contained
`taskfmt` smoke fixtures. Each package keeps its own `verify.toml` and the
trusted source, test, font, and seed inputs its checks expect.

## Order and dependencies

Apply the tasks in order. Each task depends on the completed target-repository
state from the task before it.

| Task | Slice | Depends on | Additional prerequisites |
| --- | --- | --- | --- |
| [TASK-001](TASK-001/README.md) | Bootstrap the workspace and stub binaries | Empty pgtui repository | Rust toolchain and trusted scaffold |
| [TASK-002](TASK-002/README.md) | Connection store, list, and CLI skeleton | TASK-001 | Trusted store, app, screen, and CLI tests |
| [TASK-003](TASK-003/README.md) | Connection form and save flow | TASK-002 | Trusted form, runtime, and screen tests |
| [TASK-004](TASK-004/README.md) | PostgreSQL connection and table browser | TASK-003 | Docker, seed fixtures, and trusted browser tests |
| [TASK-005](TASK-005/README.md) | Preview grid and client-side sorting | TASK-004 | Docker and trusted grid tests |
| [TASK-006](TASK-006/README.md) | Custom SQL screen | TASK-005 | Docker and trusted custom-SQL tests |
| [TASK-007](TASK-007/README.md) | Disconnect, exit behavior, and screen gallery | TASK-006 | Docker and trusted gallery tests |

## Caller-provided target and inputs

The caller supplies a Git checkout of the `pgtui` target repository, a baseline
commit, a writable progress file, the Rust toolchain, and the tools and services
required by the selected task. Run declared checks from that target repository's
root. TASK-004 through TASK-007 require Docker; each task README and precondition
check defines its full input set.

For each task, start from the committed result of its predecessor and a clean
target worktree. First copy that task's `trusted/` tree into the target checkout
with the `trusted/` prefix removed. For example,
`trusted/crates/pgtui/src/render.rs` goes to `crates/pgtui/src/render.rs`.
Commit this materialized input state, then use that commit hash as `--base`.
Only after the baseline commit exists should the executor edit the target. The
verify scope checks compare executor changes after `--base` against that task's
`writable_paths` and protected paths.

`taskfmt` reads task and verification files; it does not copy or overlay trusted
inputs, install tools, provision the target repository, or create services.
Keep trusted inputs unchanged while implementing the task.

Lint a package directly, for example:

```sh
taskfmt lint examples/TASK-001
```

After materialization and baseline commit, lint the task before execution:

```sh
TASK_DIR=/path/to/task-format/examples/TASK-001
TARGET_ROOT=/work/pgtui
PROGRESS=/progress/TASK-001.md

cp -R "$TASK_DIR/trusted/." "$TARGET_ROOT/"
git -C "$TARGET_ROOT" add crates
git -C "$TARGET_ROOT" commit -m "Add TASK-001 trusted inputs"
BASE=$(git -C "$TARGET_ROOT" rev-parse HEAD)

taskfmt lint "$TASK_DIR"
```

Once implementation work is underway, run checks-only verification as needed.
It runs task validation, scope checks, and declared commands without reading
progress or claiming completion. Checks may fail while the task is incomplete:

```sh
taskfmt verify --task-dir "$TASK_DIR" --root "$TARGET_ROOT" --base "$BASE" --no-progress
```

After the executor has finished and recorded the final progress event, run full
verification:

```sh
taskfmt verify --task-dir "$TASK_DIR" --root "$TARGET_ROOT" --progress "$PROGRESS" --base "$BASE"
```

The target repository, baseline, progress file, and task-specific prerequisites
are caller-provided. For TASK-002 through TASK-007, repeat trusted-input
materialization and baseline creation for that task after its predecessor is
committed.
