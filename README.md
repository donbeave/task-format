# task-format

`task-format` defines a versioned Markdown task contract, its local verification checks, and a
caller-maintained progress file. It ships one Rust command-line tool: `taskfmt`.

The format uses `task/v5` for task `README.md` files, `verify/v2` for `verify.toml`, and `progress/v1`
for progress Markdown. The [format reference](reference/FORMAT.md) is authoritative. The
[canonical task template](reference/task-template/) and [real task examples](examples/) show how
to apply it.

## Product boundary

One task package describes a bounded outcome. Its files have separate roles:

| File | Role |
| --- | --- |
| `README.md` | Goal, context, scope, requirements, acceptance criteria, decisions, and checklist. |
| `AGENTS.md` | Short instructions for the executor working on the task. |
| `verify.toml` | Local commands, expected results, and workspace path limits. |
| Caller-chosen progress file | Mutable coordination events, stored outside the task package. |

`taskfmt lint` validates a task package and its cross-references. `taskfmt status` reads a task and
progress file, validates the event stream, and reports derived checklist state. `taskfmt verify`
runs the declared checks and local scope checks; full verification also requires completed,
valid progress.

The executor provides the working copy, tools, prerequisites, and inputs. `taskfmt` runs there. It
does not create environments, launch agents, dispatch tasks, manage projects, provision services,
handle credentials, or promote changes. Those responsibilities belong outside
this repository. Commands declared in `verify.toml` run in the workspace and may have their own
effects; `taskfmt` does not sandbox arbitrary commands.

## Install

With the repository checked out and Rust installed, install the single binary from the root:

```sh
cargo install --path . --locked
taskfmt --help
```

## Work with a task

Create the task package from the contract files, leaving the progress seed outside it. Replace the
placeholder task ID, title, requirements, acceptance criteria, checklist, and check commands with
the real task contract. The canonical progress seed matches its example first leaf, `1.1`; update
the task ID and leaf in both the header and first event at the caller's writable progress path.

```sh
mkdir -p /task /progress
cp reference/task-template/README.md \
  reference/task-template/AGENTS.md \
  reference/task-template/verify.toml \
  /task/
cp reference/task-template/progress.md /progress/progress.md
# Edit the task files and replace TASK-000; update the external progress file's ID and first leaf.
taskfmt lint /task
```

Paths are caller choices; `/task`, `/work`, and `/progress/progress.md` are examples. Lint checks
the task and verifier configuration without running their commands or requiring progress.

During work, append valid events to the caller's progress file. Inspect its derived state at any
time:

```sh
taskfmt status --task-dir /task --progress /progress/progress.md
taskfmt status --task-dir /task --progress /progress/progress.md --json
```

Status shows task identity, state, current leaf, per-item progress, completed and total leaves,
and a whole-number percentage. The percentage is `100 × completed leaves / total leaves`, rounded
to the nearest whole percent with halves rounded up. Status validates and reads files; it does not
run checks or change them.

Use checks-only verification while progress is incomplete. Supply the caller's workspace baseline
commit as `TASK_BASE`. After preparing `/work` with the task's starting files and before making task
changes, create the baseline and resolve its commit ID:

```sh
git -C /work add -A
git -C /work commit -m "Task baseline"
TASK_BASE=$(git -C /work rev-parse HEAD)
```

If `/work` already has the intended baseline commit, set `TASK_BASE` from that commit instead.

```sh
taskfmt verify --task-dir /task --root /work --base "$TASK_BASE" --no-progress
```

After all checklist leaves are complete, run full verification with the progress path:

```sh
taskfmt verify \
  --task-dir /task \
  --root /work \
  --progress /progress/progress.md \
  --base "$TASK_BASE"
```

Full verification checks the task contract, the supplied baseline and workspace scope, every
declared command and expected result, and completed progress. A progress state of `DONE` or a 100%
status is a coordination claim. Only a successful full `taskfmt verify` run proves that the
declared checks passed for that workspace at that time; its final standalone `DONE` line is the
completion signal. Checks-only success reports `CHECKS PASS` and does not signal task completion.

## Examples

[`examples/README.md`](examples/README.md) lists the canonical task examples and their prerequisites.
Examples describe actual work and may require their named target repository or tools; the template's
placeholder commands must be replaced before execution.

## Development

The pinned Rust toolchain is in `rust-toolchain.toml`. From the repository root:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
```
