# Task package (schema task/v3)

One bounded task handed to one fresh coding agent in one isolated container.

```text
<task-dir>/                 mounted read-only at /task
  README.md                 contract: goal, context, preconditions, scope, R-*, AC table, D-*, static checklist
  AGENT.md                  execution protocol (same for every task); progress grammar; final report
  verify.sh                 generic gate; reads verify.config + protected.sha256 next to it
  verify.config             project-specific commands, globs, patterns
  protected.sha256          generated at dispatch by manifest.sh; workspace-relative paths
  manifest.sh               dispatch tool (not needed by the agent)
progress.md                 mounted read-write at /progress/progress.md; generated from README.md checklist
```

Container mounts: `/task` (ro), `/work` (rw, fresh copy of the fixture repo with a `baseline` tag), `/progress` (rw).

## Dispatch

1. Write `README.md` from the template. Fill the checklist; every leaf has an evidence command.
2. Write `verify.config`. `BASE_REF="baseline"`.
3. Generate `progress.md`: header (`TASK`, `STATE: IN_PROGRESS`, `CURRENT: <first leaf>`, `BASELINE: <not run>`) + verbatim copy of the checklist block + empty Log + Handoff.
4. From the fixture repo root: `manifest.sh gen -o <task-dir>/protected.sha256 <protected paths...>`.
5. Prove the gate: `verify.sh` must FAIL on the untouched fixture and PASS with a reference solution applied. A gate that cannot distinguish the two is not a gate.
6. Launch with `reference/goal-prompt.md`.

## Gate (host, after the container exits)

```sh
VERIFY_ROOT=<run>/workspace VERIFY_TASK_DIR=<run>/task-snapshot PROGRESS_FILE=<run>/progress.md \
  <run>/task-snapshot/verify.sh
```

Exit 0 and last line `DONE` is the only pass signal. The agent's own report is never load-bearing.

## Author checklist (to become a linter)

- One outcome, one repo, one subsystem, no open decisions, no dependency on prior chat.
- Every `P-*` has a command. Every `AC-*` row has an evidence command and expected result.
- Checklist: IDs unique and contiguous; depth matches indentation (0/4/8/12 spaces = 1-4 components); max depth 4; 5-20 leaves; every leaf has evidence; no single-child levels added just for depth.
- `expected_paths` covers what the solution touches; `protected_paths` covers tests/fixtures the agent must not weaken.
- `verify.sh` fails on baseline, passes on the reference solution.
- `README.md` under ~2,500 tokens after placeholders are replaced.
