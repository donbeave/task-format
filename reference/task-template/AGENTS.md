# Execution protocol

Your goal is to fully implement this task: `/task/README.md`.

`/task/` is read-only: it holds only the task (`README.md`, this protocol, `verify.toml`, optionally `decisions.md`) and nothing of yours. Keep your state in `/progress/progress.md`. Do all work in `/work/`.

## Files

| Path | Access | Purpose |
| --- | --- | --- |
| `/task/README.md` | read-only | The contract: goal, requirements, acceptance criteria, decisions, checklist. |
| `/task/decisions.md` | read-only, optional | Full text of the fixed decisions; binding when present. |
| `/task/verify.toml` | read-only | Gate inputs (commands, patterns, scope whitelist); never edit. |
| `/progress/progress.md` | read-write | Your only state file: checklist copy + log + handoff. |
| `/work/` | read-write | The repository. All code changes happen here. |

The completion gate is `taskfmt verify` (binary baked into the image, read-only). Run it from `/work`; never modify or bypass it. `$TASKFMT_BASE` is the scope base commit.

## Protocol

1. Read `/task/README.md` fully (and `/task/decisions.md` if present), then the files listed under "Read before editing" (orientation only, not rules).
2. If `## Log` in `/progress/progress.md` is non-empty you are resuming: read it, run `git status` and `git diff --stat`, and continue from `CURRENT:`. After compaction or any resume, re-read `/task/README.md` and `progress.md` before editing; never reconstruct state from memory.
3. State in the transcript: task ID, the one-sentence goal, the acceptance-criterion IDs, and the first leaf you will work on.
4. Run every precondition command. If one fails, write `STATE: BLOCKED` and the failing command to `progress.md`, then emit the final report with `STATUS: BLOCKED`. Do not work around a failed precondition.
5. Work the checklist leaves one at a time, in ID order unless `README.md` states a different dependency order. Before starting a leaf set `CURRENT: <id>` in `progress.md`.
6. Mark a leaf `[x]` only after running its evidence command and seeing the stated result. Append one log line with the command and observed result. If later work invalidates it, set it back to `[ ]` and log `REOPENED`. A check that passes and fails on the same tree is FAILED evidence: log it and stop `NEEDS_REPLAN`, naming the command.
7. Mark a parent `[x]` only when every child is `[x]` and, if it states its own `evidence:`, only after running and logging that too (children alone do not suffice); without evidence it rolls up. Parents are never `CURRENT`.
8. When all leaves except the gate leaf are done: (a) run `taskfmt verify --progress ""` from `/work` (progress check skipped); fix and rerun until it exits 0 with last line `DONE`. (b) Set `STATE: DONE`, `CURRENT: NONE`, mark the remaining leaves `[x]`, run `taskfmt verify` (full, with progress check); its complete output is the transcript evidence. Emit the final report.

## progress.md grammar

```text
---
TASK: TASK-000
STATE: IN_PROGRESS            # IN_PROGRESS | DONE | BLOCKED | NEEDS_REPLAN
CURRENT: 2.1.1                # one unchecked leaf ID, or NONE
BASELINE: <command> -> <observed result>
---

<!-- checklist:start -->
<verbatim copy of the README.md checklist; only the [ ]/[x] tokens change>
<!-- checklist:end -->

## Log
- 2.1.1 | DONE | cargo test expired_refresh_token -> exit 0, 1 passed
- 2.1.2 | FAILED | cargo test expired_error_code -> assertion at rotate.rs:88; next: guard the increment
- 2.1.2 | REOPENED | evidence invalidated by 2.3 removal

## Handoff
CURRENT_FAILURE: <exact failing check or none>
DECISIONS: <incidental choices or none>
```

Rules: never change checklist text, IDs, order, or indentation — only the three-character checkbox token. Never change `TASK:`. `BASELINE:` must hold the real command and observed result before `DONE`; the gate rejects `<not run>`. Log is append-only; log statuses are `DONE | FAILED | REOPENED | BLOCKED`. Missing work is not a new checklist line; it is `NEEDS_REPLAN`.

## Prohibited

- Editing anything under `/task/`, or modifying/replacing the `taskfmt` binary.
- Deleting, skipping, weakening, or rewriting a failing test or check to make it pass.
- Special-casing known fixtures or verifier inputs.
- Suppressing errors, warnings, lint rules, type checks, or exit codes.
- Changing any file outside `expected_paths` (the scope whitelist in `/task/README.md`); the gate rejects every other path.
- Changing user-visible behavior no `R-*`/`AC-*` names, even inside `expected_paths`; note it under `FOLLOW_UP`.
- Claiming `DONE` without a `taskfmt verify` run in this session whose output is in the transcript.

## Stop conditions

- `BLOCKED`: environment or dependency only — a precondition or external condition you cannot fix within scope is false (missing dependency, credentials, infrastructure). A precondition command that errors (rc 127 etc.) is `BLOCKED` too, named `precondition-broken` in `FOLLOW_UP`.
- `NEEDS_REPLAN`: satisfying the task requires changing its goal, acceptance criteria, fixed decisions, scope, or checklist; requirements contradict; a material design decision is unresolved; or the unblock itself needs such a change.
- `INCOMPLETE`: the turn or budget cap is reached first. Leave `STATE: IN_PROGRESS`, fill the handoff, report `STATUS: INCOMPLETE`.
- Do not spin. When no evidence-backed action remains, stop with the matching status, keep the current leaf unchecked, and report what was tried and the smallest decision or dependency needed to resume.

## Turn signal

At the end of every turn print one line:

```text
GOAL_PROGRESS task=TASK-000 state=<STATE> current=<ID|NONE> done_this_turn=<IDs|none> blocked=<ID|none>
```

## Final report

Last thing you print. Exactly this shape:

```text
STATUS: DONE | BLOCKED | NEEDS_REPLAN | INCOMPLETE
TASK: TASK-000
SUMMARY: <what changed, or why execution stopped and what was tried>
ACCEPTANCE:
- AC-001: PASS | FAIL | NOT_RUN — <command and observed result>
- AC-002: ...
VERIFY: command=taskfmt verify exit=<n|NOT_RUN> last_line=<DONE|other|NOT_RUN>
CHANGED:
<verbatim `git diff --no-renames --name-status $TASKFMT_BASE`, then the untracked lines of `git status --porcelain --untracked-files=all`; not recall>
DEVIATIONS: none | <list>
FOLLOW_UP: none | <smallest decision, dependency, or split needed>
GOAL_RESULT task=TASK-000 status=<STATUS>
```
