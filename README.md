# task-format

`task-format` defines one versioned Markdown task format and ships one Rust CLI, `taskfmt`, to
validate task packages, read caller-maintained progress, and run declared local checks.

The repository keeps three formats:

- `task/v5`: a task contract in `README.md`.
- `verify/v2`: local checks and scope rules in `verify.toml`.
- `progress/v1`: a caller-written event log stored outside the task package.

[`reference/FORMAT.md`](reference/FORMAT.md) is authoritative. [`reference/task-template/`](reference/task-template/)
contains the copyable seed. [`examples/`](examples/) contains real task packages. The template has
placeholders; replace them before using it as a task for real work.

## What taskfmt does

`taskfmt` has exactly three functional commands:

```text
taskfmt lint TASK_DIR [--json]
taskfmt status --task-dir TASK_DIR --progress PROGRESS_FILE [--json]
taskfmt verify --task-dir TASK_DIR --root WORKSPACE --base BASE (--progress PROGRESS_FILE | --no-progress)
```

- `lint` validates one task package and its cross-references. It does not run checks or read
  progress.
- `status` validates the task and progress file, then reports event-derived checklist state,
  current item, completed leaves, total leaves, and percentage. It only reads files.
- `verify` validates the package, compares workspace changes with the caller-supplied baseline and
  scope rules, runs declared checks, and checks progress during full verification.

Taskfmt is a format and local verification tool. The caller supplies an existing workspace,
baseline commit, progress file, tools, and other prerequisites. It does not create containers,
launch agents, dispatch tasks, acquire credentials, provision services, or manage task execution
outside the local verification process. Commands in `verify.toml` can have side effects; taskfmt
does not sandbox arbitrary commands.

## Create and verify a task

From the repository root, copy the three task files into a new package. Keep progress at a separate
caller-writable path.
Replace the template IDs, requirements, acceptance criteria, checks, and command placeholders with
the task's actual values. Replace `writable_paths` in `verify.toml` with the exact relative files and
directories the task may modify; it is the scope allowlist. Set the initial progress leaf to the
first checklist leaf.

```sh
TASK_DIR=/path/to/task
WORKSPACE=/path/to/workspace
TASK_ID=TASK-042
FIRST_LEAF=1.1
PROGRESS_FILE=/path/to/progress/${TASK_ID}.md

mkdir -p "$TASK_DIR" "$(dirname "$PROGRESS_FILE")"
cp reference/task-template/README.md reference/task-template/AGENTS.md \
  reference/task-template/verify.toml "$TASK_DIR/"
cp reference/task-template/progress.md "$PROGRESS_FILE"
PROGRESS_TMP="${PROGRESS_FILE}.tmp"
sed \
  -e "s/^task: TASK-000$/task: ${TASK_ID}/" \
  -e "s/^current: 1\\.1$/current: ${FIRST_LEAF}/" \
  -e "s/^- 1 | STARTED | 1\\.1$/- 1 | STARTED | ${FIRST_LEAF}/" \
  "$PROGRESS_FILE" > "$PROGRESS_TMP"
mv "$PROGRESS_TMP" "$PROGRESS_FILE"
# Edit the task files; set TASK_ID and FIRST_LEAF to the instantiated values.

taskfmt lint "$TASK_DIR"
```

The workspace must already contain a committed baseline representing its starting state. Record
that full commit ID before implementation; taskfmt requires it and never substitutes `HEAD` for a
missing or invalid baseline.

```sh
BASE=$(git -C "$WORKSPACE" rev-parse --verify 'HEAD^{commit}')

# During work: validate the workspace and run checks without asserting completion.
taskfmt verify --task-dir "$TASK_DIR" --root "$WORKSPACE" --base "$BASE" --no-progress

# Update the caller-owned progress file, then inspect its derived state.
taskfmt status --task-dir "$TASK_DIR" --progress "$PROGRESS_FILE"

# After every checklist leaf is recorded complete, run full verification.
taskfmt verify --task-dir "$TASK_DIR" --root "$WORKSPACE" \
  --progress "$PROGRESS_FILE" --base "$BASE"
```

The caller or executor writes the progress file. Taskfmt has no initialization or progress-writing
command. A `DONE` progress state and `100%` status are coordination claims, not proof that checks
passed. Checks-only verification ends with `CHECKS PASS`; only successful full verification ends
with a standalone `DONE` line.

## Examples

[`examples/README.md`](examples/README.md) describes the authentic `pgtui` task sequence, target
workspace requirements, and caller-provided inputs. Examples are task packages, not a self-running
campaign. [`testdata/smoke-task/`](testdata/smoke-task/) is separate synthetic CLI test data; its
no-op checks do not verify real work and it is not an authentic target example.

## Development

The pinned Rust toolchain is in [`rust-toolchain.toml`](rust-toolchain.toml). From the repository
root:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release --bin taskfmt
```

To install the CLI from this checkout:

```sh
cargo install --path . --locked
```
