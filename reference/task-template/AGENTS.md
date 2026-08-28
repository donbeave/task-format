# Execution protocol

You are implementing exactly one task: `/task/README.md`. This protocol is the same for every task. Read it once.

## Files

| Path | Access | Purpose |
| --- | --- | --- |
| `/task/README.md` | read-only | The contract: goal, requirements, acceptance criteria, decisions, checklist. |
| `/task/AGENTS.md` | read-only | This protocol. `/task/CLAUDE.md` is a symlink to it. |
| `/task/verify.sh` | read-only | The completion gate. Run it; never edit it. |
| `/progress/progress.md` | read-write | The only file you maintain for state. Checklist copy + log + handoff. |
| `/work/` | read-write | The repository. All code changes happen here. |

## Protocol

1. Read `/task/README.md` fully, then the repository instructions (`AGENTS.md`; `CLAUDE.md` is a symlink to it), then the files listed under "Read before editing".
2. If `/progress/progress.md` has `STATE: IN_PROGRESS` you are resuming: read it, run `git status` and `git diff --stat`, and continue from `CURRENT:`. Do not reconstruct state from memory.
3. State in the transcript: task ID, the one-sentence goal, the acceptance-criterion IDs, and the first leaf you will work on.
4. Run every precondition command. If one fails, write `STATE: BLOCKED` and the failing command to `progress.md`, then emit the final report with `STATUS: BLOCKED`. Do not work around a failed precondition.
5. Work the checklist leaves one at a time, in ID order unless `README.md` states a different dependency order. Before starting a leaf set `CURRENT: <id>` in `progress.md`.
6. Mark a leaf `[x]` only after running its evidence command and seeing the stated result. Append one log line with the command and observed result. If later work invalidates it, set it back to `[ ]` and log `REOPENED`.
7. Mark a parent `[x]` only when every child is `[x]`. Parents are never `CURRENT`.
8. When all leaves except the gate leaf are done, run `/task/verify.sh` from `/work`. Fix failures it reports; rerun until it exits 0 with last line `DONE`. Then set `STATE: DONE`, `CURRENT: NONE`, mark the remaining leaves, and emit the final report.

## progress.md grammar

```text
TASK: TASK-000
STATE: IN_PROGRESS            # IN_PROGRESS | DONE | BLOCKED | NEEDS_REPLAN
CURRENT: 2.1.1                # one unchecked leaf ID, or NONE
BASELINE: <command> -> <observed result>

<!-- checklist:start -->
<verbatim copy of the README.md checklist; only the [ ]/[x] tokens change>
<!-- checklist:end -->

## Log
- 2.1.1 | DONE | cargo test expired_refresh_token -> exit 0, 1 passed
- 2.1.2 | FAILED | cargo test expired_error_code -> assertion at rotate.rs:88; next: guard the increment
- 2.1.2 | REOPENED | evidence invalidated by 2.3 removal

## Handoff
NEXT: 2.1.2 — guard the counter increment
CURRENT_FAILURE: <exact failing check or none>
DECISIONS: <incidental choices or none>
```

Rules: never change checklist text, IDs, order, or indentation — only the three-character checkbox token. Never change `TASK:`. `BASELINE:` must hold the real command and observed result before `DONE`; the gate rejects `<not run>`. Log is append-only; log statuses are `DONE | FAILED | REOPENED | BLOCKED`. Missing work is not a new checklist line; it is `NEEDS_REPLAN`.

## Prohibited

- Editing anything under `/task/` or any `protected_paths` file. The gate hashes them.
- Deleting, skipping, weakening, or rewriting a failing test or check to make it pass.
- Special-casing known fixtures or verifier inputs.
- Suppressing errors, warnings, lint rules, type checks, or exit codes.
- Changing files outside `expected_paths` without a task-specific reason stated in the log.
- Claiming `DONE` without a `/task/verify.sh` run in this session whose output is in the transcript.

## Stop conditions

- `BLOCKED`: the task is valid but a precondition or external condition you cannot fix within scope is false (missing dependency, credentials, infrastructure).
- `NEEDS_REPLAN`: satisfying the task requires changing its goal, acceptance criteria, fixed decisions, scope, or checklist; requirements contradict; a material design decision is unresolved.
- Do not spin. When no evidence-backed action remains, stop with the matching status, keep the current leaf unchecked, and report the smallest decision or dependency needed to resume.

## Turn signal

At the end of every turn print one line:

```text
GOAL_PROGRESS task=TASK-000 state=<STATE> current=<ID|NONE> done_this_turn=<IDs|none> blocked=<ID|none>
```

## Final report

Last thing you print. Exactly this shape:

```text
STATUS: DONE | BLOCKED | NEEDS_REPLAN
TASK: TASK-000
SUMMARY: <what changed, or why execution stopped>
ACCEPTANCE:
- AC-001: PASS | FAIL | NOT_RUN — <command and observed result>
- AC-002: ...
VERIFY: command=/task/verify.sh exit=<n|NOT_RUN> last_line=<DONE|other|NOT_RUN>
CHANGED:
- <path> — <reason>
DEVIATIONS: none | <list>
FOLLOW_UP: none | <smallest decision, dependency, or split needed>
GOAL_RESULT task=TASK-000 status=<STATUS>
```
