# Task harness (schema task/v3)

Everything required to author, dispatch, execute and gate a task package. The package itself is the research object and lives pure in `reference/task-template/`; this folder holds the tooling around it.

## Layout

```text
reference/task-template/    the task package — pure template, exactly what ships to the agent
  README.md                 contract: goal, context, preconditions, scope, R-*, AC table, D-*, static checklist
  AGENTS.md                 execution protocol (same for every task); progress grammar; final report
  CLAUDE.md -> AGENTS.md    symlink, so Claude Code auto-loads the protocol from /task (--add-dir) and Codex reads AGENTS.md
  verify.sh                 generic gate; reads verify.config + protected.sha256 next to it
  verify.config             project-specific commands, globs, patterns
  protected.sha256          generated at dispatch by manifest.sh; workspace-relative paths

harness/                    dispatch tooling — never shipped into /task
  task-lint.sh              author checklist below, mechanically
  progress-init.sh          README.md -> initial progress.md (lints first)
  manifest.sh               generates/checks protected.sha256
  selftest.sh               proves lint + progress-init + verify.sh on a throwaway fixture
  goal-prompt.md            the prompt used to start the agent (/goal condition for Claude Code, Codex variant)
  testdata/example/         lint corpus: a filled-in task (TASK-042) used by selftest.sh

<run>/progress/progress.md  mounted read-write at /progress/progress.md; GENERATED per run, never stored
```

`manifest.sh` is not copied into task dirs at dispatch: the agent never needs it, and `/task` holds only package files (README.md, AGENTS.md, verify.sh, verify.config, protected.sha256).

`progress.md` is derived state. It is generated from `README.md` by `progress-init.sh` at dispatch and lives only in the run directory (`.gitignore` blocks it everywhere). A stored copy would be a second source of truth that drifts from the checklist.

Copy the package with `cp -a` (keeps the symlink). Container mounts: `/task` (ro), `/work` (rw, fresh copy of the fixture repo with a `baseline` tag), `/progress` (rw).

## Dispatch

1. Write `README.md` from the template. Fill the checklist; every leaf has an evidence command.
2. Write `verify.config`. `BASE_REF="baseline"`.
3. `harness/task-lint.sh <task-dir>` must print `LINT PASS`.
4. `harness/progress-init.sh <task-dir> -o <run>/progress/progress.md` — header (`TASK`, `STATE: IN_PROGRESS`, `CURRENT: <first leaf>`, `BASELINE: <not run>`) + verbatim checklist block + empty Log + Handoff. Refuses on lint failure.
5. From the fixture repo root: `harness/manifest.sh gen -o <task-dir>/protected.sha256 <protected paths...>`.
6. Prove the gate: `verify.sh` must FAIL on the untouched fixture and PASS with a reference solution applied. A gate that cannot distinguish the two is not a gate.
7. Launch with `harness/goal-prompt.md`.

## Gate (host, after the container exits)

```sh
VERIFY_ROOT=<run>/workspace VERIFY_TASK_DIR=<run>/task-snapshot PROGRESS_FILE=<run>/progress.md \
  <run>/task-snapshot/verify.sh
```

Exit 0 and last line `DONE` is the only pass signal. The agent's own report is never load-bearing. The `progress` check requires: checklist block equal to `README.md` modulo `[ ]`/`[x]`, every leaf `[x]`, parents consistent, `TASK:` equal to the README `id`, `STATE: DONE`, `CURRENT: NONE`, `BASELINE:` recorded (not `<not run>`).

## Planned container harness (not built yet)

Headed design (herdr) in `docs/research/notes/headed-herdr-harness.md`; image/mount/gate basics in `docs/research/notes/container-harness.md`. Future layout, landing here in `harness/`:

```text
images/{claude,codex}/Dockerfile   images/common/{init-firewall.sh,entrypoint.sh}
run-headed.sh                       run-headed.sh <claude|codex> <TASK-ID> [--model M] [--net api|all]  (persistent container, herdr)
attach.sh / status.sh               re-attach to the live agent; detect completion
lib/                                hash-protected.sh, gate.sh
```

Run outputs go to `experiments/runs/<ts>-<agent>-<task>/` (workspace, task-snapshot, progress.md, agent-home, transcript.jsonl, stdout.ndjson, diff.patch, verify-host.log, metrics.json).

## Author checklist (enforced by `task-lint.sh`)

- One outcome, one repo, one subsystem, no open decisions, no dependency on prior chat.
- Every `P-*` has a command. Every `AC-*` row has an evidence command and expected result.
- Checklist: IDs unique and contiguous; depth matches indentation (0/4/8/12 spaces = 1-4 components); max depth 4; 5-20 leaves; every leaf has evidence; no single-child levels added just for depth; every `AC-*` referenced; last leaf is the `verify.sh` gate.
- `expected_paths` covers what the solution touches and equals `ALLOWED_GLOBS`; every `protected_paths` entry matches a `DENIED_GLOBS` entry.
- `verify.sh` fails on baseline, passes on the reference solution (not lintable; step 6).
- `README.md` under ~2,500 tokens after placeholders are replaced (lint warns above 10,000 bytes).

`selftest.sh` exercises the tools end to end (lint accept/reject, generated progress shape, gate FAIL on fresh/tampered progress, PASS only on the complete DONE file). Run it after touching any script here.
