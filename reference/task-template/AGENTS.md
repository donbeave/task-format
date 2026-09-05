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

The task README uses canonical typed acceptance blocks. Each non-gate `AC-*` block has `Type`,
`Class`, real `Covers` requirement IDs, exact `Evidence` and `Expected` fields, and one exact
` ```gherkin ` fence containing only constrained Given/When/Then-shaped behavior. A gate block
has `Type: gate`, exact `Evidence`/`Expected`, and no body. These are task metadata, not Cucumber
feature files and have no runtime step definitions. Every `Evidence` field uses one exact
` ```sh ` fence representing one command, regardless of command length.

## Protocol

1. Read `/task/README.md` fully (and `/task/decisions.md` if present), then the files listed under "Read before editing".
2. If `## Log` in `/progress/progress.md` is non-empty you are resuming: read it, run `git status` and `git diff --stat`, and continue from `CURRENT:`. `/progress/progress.md` on disk is the ONLY authority for what is done; any summary of it that appears in your context — including one you or the runtime generated — is a claim to be checked against the file, never a substitute for it. Re-read the file immediately before any edit that changes `STATE:`, `CURRENT:` or a checkbox, and again before the final report. Re-read `/task/README.md` before editing after any resume or context compaction. Never reconstruct state from memory.
3. State in the transcript: task ID, the one-sentence goal, the acceptance-criterion IDs, and the first leaf you will work on.
4. Run every precondition command. If one fails, write `STATE: BLOCKED` and the failing command to `progress.md`, then emit the final report with `STATUS: BLOCKED`. Do not work around a failed precondition.
5. Work the checklist leaves one at a time, in ID order unless `README.md` states a different dependency order. Before starting a leaf set `CURRENT: <id>` in `progress.md`.
6. Mark a leaf `[x]` only after running its evidence command and seeing the stated result — and, having seen it, mark it in the same edit that appends its log line, before you start the next leaf. The log and the checkbox are one record: at every point a leaf's box is `[x]` if and only if its latest log row is `DONE`. If later work invalidates it, set the box back to `[ ]` and log `REOPENED` in the same edit. Before you emit the final report, in any terminal state, re-read `/progress/progress.md` and reconcile the two; a leaf whose box and latest row disagree is a defect in your own reporting — fix it there. A check that passes and fails on the same tree is FAILED evidence: log it and stop `NEEDS_REPLAN`, naming the command.
7. Mark a parent `[x]` only when every child is `[x]` and, if it states its own `evidence:`, only after running and logging that too (children alone do not suffice); without evidence it rolls up. Parents are never `CURRENT`.
8. When all leaves except the gate leaf are done: (a) run `taskfmt verify --progress ""` from `/work` (progress check skipped); fix and rerun until it exits 0 with last line `DONE`. (b) Set `STATE: DONE`, `CURRENT: NONE`, mark the remaining leaves `[x]`, appending each one's `DONE` log row in the same edit (the gate leaf's row is (a)'s command and observed result), run `taskfmt verify` (full, with progress check); its complete output is the transcript evidence. Emit the final report.

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

- `BLOCKED`: a precondition command exited non-zero, or an environment or dependency condition outside `expected_paths` is false (missing dependency, credentials, infrastructure). Do not install, upgrade, downgrade or substitute anything the environment is expected to provide — a toolchain or one of its components, a runtime, a system package, a container image — whether or not you are able to, and whether or not it changes a file; adding or pinning a dependency of the code under test that a file inside `expected_paths` declares, where the task requires it, is task work and is not covered by this rule. A precondition command that errors (rc 127 etc.) is `BLOCKED` too.
- `NEEDS_REPLAN`: satisfying the task requires changing its goal, acceptance criteria, fixed decisions, scope, or checklist; requirements contradict; a material design decision is unresolved; or the unblock itself needs such a change.
- `INCOMPLETE`: the turn or budget cap is reached first. Leave `STATE: IN_PROGRESS`, fill the handoff.
- Do not spin. A leaf with no evidence-backed action left is logged `FAILED` with the command and the observed result and stays `[ ]`; then move to the next leaf that does not depend on it. Take a terminal only when no leaf anywhere has an evidence-backed action left — or at once where rule 4 or rule 6 says to stop — and report every `FAILED` leaf, what was tried, and the smallest decision or dependency needed to resume. If you continued past a failed leaf, say so under `DEVIATIONS`. Retrying a leaf whose failure you have already diagnosed is spinning.

## Turn signal

At the end of every turn EXCEPT the one that carries the final report, print one line:

```text
GOAL_PROGRESS task=TASK-000 state=<STATE> current=<ID|NONE> done_this_turn=<IDs|none> blocked=<ID|none>
```

On the turn that carries the final report, print this line immediately BEFORE the report and print nothing after the report's `GOAL_RESULT` line. `GOAL_RESULT` is the last line of the session, in every terminal state, without exception.

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
