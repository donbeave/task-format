# task-format

`task-format` is a research harness for one practical question:

> Can a structured task package make AI coding work more predictable, bounded, and independently verifiable?

It also provides a local filesystem-backed progress monitor for projects, groups,
and dependency-linked tasks. The Rust API and Bun/TanStack Start browser reuse
taskfmt's task contracts, progress, and authoritative verification lifecycle.
See [monitoring setup and schemas](docs/monitoring.md),
[example projects](docs/monitoring-examples.md), and
[installed toolchain versions](docs/monitoring-versions.md).
The [folder discovery demo](docs/monitoring-demo.md) shows all seven existing
experimental tasks discovered from `tasks/demo/pgtui/` by the running application.

## Install locally

From the repository root, install the `taskfmt` binary with Cargo:

```sh
cargo install --path harness --locked
```

Cargo installs the binary to `~/.cargo/bin`. Ensure that directory is on your `PATH`, then run:

```sh
taskfmt --help
```

## Why this exists

An AI coding agent turns prose into edits, checks, and a completion claim. If scope, decisions, or proof are unclear, it can drift into unrelated work or report success without solving the problem. Faster agents make this ambiguity more expensive, not less.

This project treats task writing as an engineering variable. It gives an agent one explicit contract and checks the result at a caller-provided execution boundary.

## What this project is—and is not

It is a versioned task format, a Rust harness, and an experiment corpus for testing task-package design.

It is not a model leaderboard, a universal prompting recipe, or proof that the current format is optimal. The current `TASK-001`–`TASK-007` series validates the harness and package lifecycle; it is not yet a measured format comparison.

## The task package

One package describes one bounded, observable outcome:

- `README.md` — the binding goal, context, requirements, scope, acceptance criteria, fixed decisions, and checklist;
- `AGENTS.md` — execution protocol and progress rules;
- `verify.toml` — commands, path limits, and completion gate;
- `trusted/` — planner-owned tests or fixtures outside the agent's edit scope.

The package is read-only during a run. Progress is derived state stored outside the package and is never itself proof of completion.

## Trust model

The harness protects the experiment in layers:

1. A fresh workspace and agent session prevent previous state from contaminating a run.
2. A recorded baseline anchors scope checks to a known Git tree.
3. Trusted verification material is overlaid before execution and kept outside `verify.toml`'s writable paths.
4. `taskfmt verify` executes the declared checks, expected matchers, progress validation, and scope check.
5. The host freezes one complete candidate tree, gates that exact tree, and records its evidence.
6. Promotion creates and pushes a commit directly from the recorded tree with an expected-parent lease. The host verdict, not the agent's report, decides success.

## Running experiments

Run commands from the repository root. Docker must be running, and the agent credentials
referenced by `experiment.toml` must be available.

### First-time setup

Install the host binary, preload the pinned PostgreSQL prerequisite image, and build the agent
images:

```sh
cargo install --path harness --locked
taskfmt preload --auto
taskfmt build-images --auto
```

`taskfmt build-images` defaults to `--agent all`, so it builds the shared taskfmt/base images plus
both `harness-claude` and `harness-codex`. Use `--agent claude` or `--agent codex` only when you
want to build one agent layer. You do not need to run `docker build` directly. Re-run
`build-images` after changing harness Rust code so the binary on the host and the binary inside
the image match.

Dispatch also checks that the image contains the preloaded PostgreSQL tarball before launching a
persistent run. If a stale image is missing it, rebuild with `taskfmt preload --auto` followed by
`taskfmt build-images --agent all --auto`.

Image building and runtime selection are separate. Build both images once, then choose the agent
profile for each run with `--agent`.

The current image toolchain is:

| Component | Version |
| --- | --- |
| Claude Code | `2.1.261` |
| Codex CLI | `0.153.4` |
| Rust | `1.98.1` |
| Node.js LTS | `24.20.0` |
| Herdr | `0.8.2` |
| PostgreSQL | `18.6` |
| Debian | `13.6` (Trixie) |

Rebuild the images when upgrading these pins. The build prints all four generated image tags and
each agent Dockerfile runs its CLI version check during the build.

### Use Claude with GLM-5.3-Flash

The repository defines the `zai-flash` profile for Claude through Z.ai's Anthropic-compatible API.
Create the token file expected by `experiment.toml`:

```sh
mkdir -p ~/.config/taskfmt
chmod 700 ~/.config/taskfmt
$EDITOR ~/.config/taskfmt/zai-flash.token
chmod 600 ~/.config/taskfmt/zai-flash.token
```

Put only the Z.ai API token in that file. Build the Claude image, then select the `zai-flash`
profile when running tasks:

```sh
taskfmt build-images --agent claude --auto
taskfmt run \
  --task TASK-001 \
  --repo <repository-url> \
  --agent zai-flash \
  --wait
```

For a series:

```sh
taskfmt experiment \
  --tasks 1-3 \
  --repo <repository-url> \
  --agent zai-flash \
  --auto
```

`--agent claude` selects the image family during image building; `--agent zai-flash` selects the
configured Claude/GLM-5.3-Flash profile during execution. Because `zai-flash` is the default
profile, the execution commands may omit `--agent zai-flash`.

### Run one task

Lint the task, then dispatch it to a fresh container:

```sh
taskfmt lint TASK-001
taskfmt run --task TASK-001 --repo <repository-url> --agent codex-default --wait
```

Without `--repo`, `taskfmt run` creates a disposable private repository after confirmation. The
`--wait` flag waits for the agent and runs the host gate. After a passing result, promote the
recorded tree:

```sh
taskfmt promote <run-id>  # only after a passing gate
```

To return immediately instead, omit `--wait`; follow the run with `taskfmt status <run-id> --wait`,
then run `taskfmt gate <run-id>` before promotion.

`taskfmt selfcheck TASK-001 <workspace>` is an optional pre-dispatch check that proves the task
gate distinguishes the untouched base from a reference solution.

### Run an ordered experiment series

`experiment` runs the repository lifecycle and, for each selected task, performs dispatch, gating,
and promotion in order. It stops on the first failure or blocked task:

```sh
taskfmt experiment \
  --tasks 1-3 \
  --repo <repository-url> \
  --agent codex-default \
  --auto
```

Omit `--repo` to create a disposable experiment repository. Use `--tasks all` or selections such
as `1-3,5` and `TASK-002..TASK-004`. Resume an interrupted series with its experiment ID:

```sh
taskfmt experiment --resume <experiment-id> --tasks all --agent codex-default --auto
```

### Run every task in one command

After first-time setup, run the complete configured task corpus sequentially with Claude and
GLM-5.3-Flash:

```sh
taskfmt experiment \
  --tasks all \
  --repo <repository-url> \
  --agent zai-flash \
  --auto
```

Omit `--repo` to let `taskfmt` create a disposable private repository. Add
`--proof-corpus /path/to/pgtui-proof.git` for a controlled campaign; the corpus is preflighted
before any runtime repository is created and pinned to the experiment state. This is one command,
not parallel execution: tasks run in order, and the experiment stops at the first failed or
blocked task. Resume the same experiment after fixing the cause:

```sh
taskfmt experiment \
  --resume <experiment-id> \
  --agent zai-flash \
  --auto
```

Use `taskfmt ps`, `taskfmt status <run-id>`, or `taskfmt attach <run-id>` to inspect a live or
completed run. A prereq failure still writes the launch manifest, so `attach` can locate the
parked container and points to `out/prereqs.log`. Run records and evidence are stored under
`experiments/runs/`.

See [harness/README.md](harness/README.md) for complete flags, safety rules, configuration, and
development checks.

## Research question and method

The research compares task-package writing, not software outcomes. Change one meaningful variable—such as checklist shape, rule placement, context order, or verification presentation—while holding the outcome, repository, trusted checks, gate, image, agent profile, and runtime limits fixed.

Each observation is a fresh run with a recorded baseline, an independent gate, and retained artifacts. A single favorable run is a signal, not a finding.

Measure gate pass rate, false-completion claims, scope violations, diff stability, retries, and rework across repeated runs. Adopt a format change only when control comparisons show a reproducible improvement without weakening verification or changing the task outcome.

## Topics and adopted decisions

| Research topic | Current decision |
| --- | --- |
| Broad work vs. bounded work | Split into coherent, independently verifiable vertical slices. |
| Executor invention vs. prepared design | Resolve consequential product, architecture, and compatibility choices before dispatch. |
| Mutable contract vs. protected contract | Keep task instructions and planner-owned proof read-only. |
| Self-report vs. independent proof | Let the host-side gate decide completion. |
| Shared mutable checks vs. trusted checks | Overlay verifier inputs outside the agent's allowed paths. |
| Reused state vs. fresh state | Start each observation from a fresh clone, container, and session. |
| Pixel-only UI proof vs. portable evidence | Prefer deterministic semantic or textual assertions for interactive behavior. |
| Repeated prose vs. enforceable rules | Put critical invariants in the harness and configuration. |

These are design decisions and hypotheses, not claims that alternatives always fail.

## Findings and open evidence

Research converged on five durable principles: bounded outcomes, settled decisions, protected inputs, independent gates, and inspectable fresh runs. `task/v5`, `verify/v2`, `taskfmt`, and seven ordered `pgtui` packages implement those ideas.

No completed ablation matrix yet proves that one wording or checklist style produces a quantitative improvement. Future claims require repeated control comparisons and their full run records.

## Repository map

| Path | Role |
| --- | --- |
| `harness/` | Rust `taskfmt` CLI and operator reference. |
| `experiments/tasks/` | Versioned task packages used as experiment inputs. |
| `experiments/fixtures/` | Shared deterministic seed data. |
| `experiments/runs/` | Generated run workspaces and evidence; Git-ignored. |
| `reference/task-template/` | Canonical `task/v5` + `verify/v2` package template. |
| `experiment.toml` | Versioned paths, images, runtime, and agent profiles. |
| `tasks/` | Hierarchical example projects, groups, and task packages. |
| `web/` | Bun-managed TanStack Start progress-monitoring frontend. |
| `docs/monitoring.md` | Monitor setup, metadata, dependency, and lifecycle reference. |

## Authority and boundaries

When documents disagree, use this order: harness code and `experiment.toml`, then the task template and live experiment packages, then this documentation. Documentation explains behavior; it does not override executable rules.

Do not alter task-package structure, task metadata, trusted verification, or fixtures while changing explanatory documentation. A task may change only paths declared by `verify.toml`'s `writable_paths`.

## License

Apache License 2.0. See [LICENSE](LICENSE).
