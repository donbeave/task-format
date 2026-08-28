# Research message 02 — Checklist / checkboxes for progress tracking

## Prompt

I think we forgot to add checklist (checkboxes) to the task template. Checkboxes is important to track the progress and understand how much of this task is already done.

We need to research best practices to use checklist/checkboxes for /goal execution to track the progress and see what's done and what's not started yet.

Checklists can be nested. To create subitem, indent the line by using four spaces or one tab before the list marker:

- [ ] 1. Main Project
    - [x] 1.1. Phase 1 complete
    - [ ] 1.2. Phase 2 pending

And it's important to have numbers to track which item is in progress by knowing its checklist number (subitems). Each subitems must have a nested structure, like 4.2.7.5 (forth level hierarchy)

## Response

Yes—the original template was missing a first-class execution checklist. The important correction is not merely adding checkboxes, but defining **how their state is interpreted, updated, verified, and protected from the executor rewriting the task.**

## Final design decision

The authoritative checklist now lives **inside `task.md`**, so both the operator and the agent can immediately see: what completed, what pending, exact numbered leaf in progress, overall verified progress, where execution stopped if blocked.

The separate `progress.md` is retained only as an **evidence and handoff log keyed by checklist IDs**. It does not contain a second checklist, because duplicate task lists would eventually diverge.

Task package:

```text
tasks/TASK-042/
├── task.md       # contract + authoritative numbered checklist
├── progress.md   # evidence and handoff records keyed by checklist ID
└── verify.sh     # trusted completion gate
```

## Why this structure is appropriate

OpenAI's execution-plan guidance requires checkboxes in the progress section, says the progress list must always represent actual state, recommends updating at every stopping point with timestamps. Distinguishes narrative milestones from granular progress tracking. ([OpenAI Developers][1])

GFM formally supports arbitrarily nested task lists, but standard checkbox syntax is binary: blank = unchecked, `x` = checked. No standard third state for "in progress." ([GitHub Pages][2])

Model used:

```text
[ ] + not named as current = PENDING
[ ] + named as current     = IN_PROGRESS
[x]                        = COMPLETED
```

Corresponds to Claude's documented task lifecycle (pending, in progress, completed) without nonstandard syntax like `[-]` or `[~]`. ([Claude][3])

## Hierarchical numbering

Every checkbox has permanent decimal-path identifier:

```markdown
- [ ] **4** Complete final verification.
    - [ ] **4.2** Validate the changed subsystem.
        - [ ] **4.2.7** Validate session rotation.
            - [ ] **4.2.7.5** Prove expired tokens cannot mutate session state.
```

Rules:

1. IDs use one to four levels: `1`, `1.2`, `1.2.3`, `1.2.3.4`.
2. Every child ID contains complete parent prefix.
3. Four ASCII spaces per nesting level; tabs prohibited.
4. IDs cannot be reordered, deleted, renamed, or reused after execution starts.
5. A fifth meaningful level means the `/goal` task should be split.
6. Planner creates complete checklist before dispatch.
7. Missing independently trackable work produces `NEEDS_REPLAN`; executor does not silently expand checklist.

Four-level limit is intentional task-sizing policy: deeper decomposition means task too broad. ([GitHub Pages][2])

## Current-item tracking

Narrowly mutable status block:

```markdown
## Execution status

- **State:** `IN_PROGRESS`
- **Current leaf item:** `4.2.7.5`
- **Leaf progress:** `9/14 (64%)`
- **Last checkpoint (UTC):** `2026-08-28T10:15:00Z`
```

Exactly one unchecked **leaf** can be current. Parent such as `4.2.7` cannot be current (roll-up outcome, not one executable checkpoint). Agent always works on deepest applicable leaf.

## Leaf-only progress calculation

```text
verified leaf progress = checked leaves / total leaves
```

Parent checkboxes excluded; otherwise same work counted repeatedly at every level.

```markdown
- [ ] **2** Implement authentication.
    - [x] **2.1** Reject invalid tokens.
    - [ ] **2.2** Rotate valid tokens.
```

= `1/2`, not 1 of 3 checkboxes.

Parent checked only when: every descendant checked; parent's aggregate outcome actually true; any parent-level verification condition succeeds.

## Evidence before completion

Checkbox not checked merely because code edited. Every executable leaf includes completion condition:

```markdown
- [ ] **3.2.1.4** Add the expired-token regression scenario
  (`R-002`, `AC-001`) — complete when
  `cargo test expired_refresh_token` exits `0`.
```

Before `[ ]` to `[x]`, agent records evidence in `progress.md`:

```text
2026-08-28T10:15:00Z | 3.2.1.4 | COMPLETED
Evidence: cargo test expired_refresh_token
Result: exit 0; 1 passed
```

([OpenAI Developers][1])

If later work invalidates evidence, item reopened `[x]` to `[ ]`, reason recorded as `REOPENED` in progress log.

## Making checklist progress visible to `/goal`

Claude Code's `/goal` evaluator does **not** independently read `task.md`, `progress.md`, or run commands. Judges completion from what Claude surfaced in conversation. ([Claude][4])

Before every `/goal` turn ends, executor prints:

```text
GOAL_PROGRESS task=TASK-042 state=IN_PROGRESS current=4.2.7.5 verified_leaves=9/14 newly_completed=4.2.7.4 blocked=none next=4.2.7.5
```

Gives evaluator/operator: exact current checklist number; verified completion count; items completed this turn; blocked IDs; next expected leaf.

Codex goal guidance similarly recommends checkpoints, short progress log, reporting current checkpoint, verified work, remaining work, blockers. ([OpenAI Developers][5])

## Protecting the task contract

Checklist updated inside `task.md`, so file no longer protected by whole-file hash. Two explicitly marked mutable regions:

```text
goal-task:mutable-status:start ... goal-task:mutable-status:end
goal-task:checkbox-state:start ... goal-task:checkbox-state:end
```

Only valid changes: status values inside status region; checkbox tokens `[ ]` <-> `[x]`.

Executor must not change: checklist text, IDs, nesting/indentation, ordering, requirement/acceptance references, completion conditions, any static prose.

Outer verifier normalizes mutable status values and checkbox tokens before hashing/diffing. Allows progress updates while detecting attempts to weaken/reinterpret.

## Updated `/goal` invocation

```text
/goal Implement exactly the task in @task.md.

Treat its goal, requirements, acceptance criteria, fixed decisions,
scope, checklist text and IDs, and verification contract as immutable.
Update only the marked execution-status values, checkbox tokens, and
the progress log.

Execute checklist leaves in numeric depth-first order. Keep exactly one
unchecked leaf as Current leaf item. Before starting a leaf, set its
exact hierarchical ID as current. Check it only after its stated
completion evidence succeeds, record timestamped evidence in the
progress log, and then roll up any completed parents.

Before every turn ends, print the required GOAL_PROGRESS line.

Continue until every checklist leaf and parent is checked consistently,
every acceptance criterion has direct evidence, the final diff is
within scope, static task content and protected inputs are unchanged,
and ./verify.sh exits 0 with final stdout line exactly DONE.

If a precondition fails, return BLOCKED at the exact checklist leaf ID.
If required work is missing from the checklist or completion requires
changing scope, acceptance criteria, design decisions, repository, or
primary subsystem, return NEEDS_REPLAN. Do not add, delete, reorder,
rewrite, or renumber checklist items.
```

## Sources

[1]: https://developers.openai.com/cookbook/articles/codex_exec_plans
[2]: https://github.github.com/gfm/
[3]: https://code.claude.com/docs/en/agent-sdk/todo-tracking
[4]: https://code.claude.com/docs/en/goal
[5]: https://developers.openai.com/codex/use-cases/follow-goals/

---

## Reference file: GOAL-PROGRESS-LOG-TEMPLATE.md (identical to package progress.md)

```markdown
# TASK-000 progress and evidence log

> This is mutable evidence and handoff state for the authoritative checklist in `task.md`. Do not create a second checklist here and do not change the task contract through this log.

```text
TASK: TASK-000
STATE: NOT_STARTED | IN_PROGRESS | VERIFYING | DONE | BLOCKED | NEEDS_REPLAN
CURRENT_ITEM: <hierarchical leaf ID or NONE>
LEAF_PROGRESS: <completed>/<total> (<percent>%)
BASELINE: <command and result>

CHECKPOINTS:
- <UTC timestamp> | <item ID> | COMPLETED | <command/artifact and concise evidence>
- <UTC timestamp> | <item ID> | IN_PROGRESS | completed: <fact>; remaining: <fact>
- <UTC timestamp> | <item ID> | REOPENED | previous evidence invalidated by: <fact>
- <UTC timestamp> | <item ID> | FAILED | <exact failure and next evidence-based action>
- <UTC timestamp> | <item ID> | BLOCKED | <exact blocker and required unlock>

CURRENT_FAILURE: <exact failing check or none>
LAST_ACTION: <what changed and what the evidence showed>
NEXT_ACTION: <single best next action, naming its checklist ID>
DECISIONS: <task-local incidental decisions or none>
```

## Evidence rules

- Add an entry immediately before changing a leaf from `[ ]` to `[x]`.
- Every checked leaf must have one corresponding `COMPLETED` entry with evidence.
- A partially completed leaf remains unchecked and receives an `IN_PROGRESS` entry.
- If later work invalidates prior evidence, reopen the leaf in `task.md` and add a `REOPENED` entry here.
- Use ISO-8601 UTC timestamps.
- Keep output excerpts concise; point to full logs or artifacts by path.
```

## Reference file: package README.md

```markdown
# `/goal` task package template

tasks/TASK-000/
├── task.md       # protected contract + narrowly mutable status/checklist
├── progress.md   # mutable evidence and handoff log keyed by checklist IDs
└── verify.sh     # trusted canonical completion gate

The checklist in `task.md` is authoritative. During execution, only the marked status values and checkbox tokens may change. `progress.md` records evidence; it must not duplicate or redefine the checklist.
```

## Reference file: GOAL-TASK-TEMPLATE.md v2 (schema goal-task/v2; identical to package task.md)

```markdown
---
schema: goal-task/v2
id: TASK-000
title: "<imperative title describing one observable outcome>"
kind: implementation # implementation | bugfix | refactor | migration | test | docs | investigation
parent: "<roadmap, issue, decision record, or plan reference>"
depends_on: [] # task IDs that must already be complete
repository_root: "."
base_ref: "<expected branch or commit, when relevant>"
subsystem: "<one primary subsystem>"
progress_log: ".goal/progress/TASK-000.md"
task_contract:
  path: "<repository-relative path to this task file>"
  allowed_mutations:
    - "values inside the marked Execution status block"
    - "checkbox markers inside the marked Execution checklist block: [ ] <-> [x]"
checklist:
  max_depth: 4
  indentation_spaces: 4
  progress_basis: leaf_items
  max_active_leaf_items: 1
  default_order: depth_first
verification:
  command: "./verify.sh"
  success_exit_code: 0
  success_last_line: "DONE"
protected_paths:
  - "./verify.sh"
  # Add trusted tests, fixtures, snapshots, schemas, policies, or other inputs
  # the executor must not weaken. Do not list the whole task file here; its
  # static content is protected by the task-contract mutation policy above.
---

# TASK-000 — <imperative title describing one observable outcome>

> **Protected execution contract with a narrowly mutable progress surface**
>
> This file is the source of truth for one `/goal` run. Its goal, requirements, acceptance criteria, decisions, checklist text, checklist IDs, hierarchy, order, and verification contract are immutable during execution.
>
> The executor MAY change only:
>
> 1. Values inside the marked **Execution status** block.
> 2. Checkbox markers inside the marked **Execution checklist** block, and only between `[ ]` and `[x]`.
> 3. The separate mutable progress log named in `progress_log`.
>
> The executor MUST NOT add, delete, rewrite, reorder, renumber, re-indent, or otherwise alter checklist items. If the task or checklist is contradictory, incomplete, or cannot be satisfied within its boundaries, report `BLOCKED` or `NEEDS_REPLAN`; do not reinterpret or broaden it.

## Goal

<One sentence describing the single state that must become true. Use one primary outcome, not a list of work items.>

## Observable outcome

After this task, <user/operator/developer> can <observable behavior that was not possible or correct before>.

The result is observable by <specific command, API request, UI interaction, generated artifact, test scenario, metric, or other evidence>.

## Why this task exists

<Explain the current incorrect, missing, unsafe, slow, or inconsistent behavior and why this bounded change is required. Include only context needed for this task, not the whole roadmap.>

Current behavior:

- <Concrete fact about the current implementation or failure.>
- <Concrete consequence.>

Desired behavior:

- <Concrete post-change behavior.>

## Starting point and required context

Read the following sources before editing, in this order:

1. `<path/to/repository/instructions>` — <why it applies>.
2. `<path/to/relevant/module>` — <what the executor must understand>.
3. `<path/to/reference/implementation-or-test>` — <pattern or contract to follow>.
4. `<path/to/decision-record-or-spec>` — <already-decided requirement; do not reopen>.

Relevant code flow:

<Briefly explain how the named files/modules interact. Define non-obvious terms. A fresh agent must not need the parent roadmap or a previous chat to understand this task.>

Baseline or reproduction:

```sh
<exact command, run from repository_root>
```

Expected before this change:

```text
<concise expected failure, missing behavior, or baseline result>
```

## Preconditions

All preconditions must be true before implementation begins:

- **P-001:** <dependency task or required repository state>.
- **P-002:** <required tool, fixture, service, schema, or environment state>.
- **P-003:** <assumption that has already been decided and verified>.

If a precondition is false, do not work around it by expanding scope. Report `BLOCKED` with the failed precondition and evidence.

## Scope boundaries

### In scope

- <One coherent behavior or vertical slice.>
- <Tests and documentation directly required to prove that behavior.>
- <Necessary cleanup of the replaced implementation inside this boundary.>

### Out of scope

- <Adjacent feature or independent improvement.>
- <Cross-subsystem or cross-repository work that belongs in another task.>
- <Unrelated cleanup, dependency upgrades, formatting sweeps, or speculative abstractions.>

### Expected touch points

These paths are orientation, not permission to ignore other necessary task-local files:

- `<path>` — <expected change>.
- `<path>` — <expected change>.

### Forbidden or protected touch points

- Static content of this task contract outside the explicitly mutable fields.
- `./verify.sh` and all paths listed in `protected_paths`.
- `<production secrets, generated vendor code, unrelated subsystem, deployment configuration, etc.>`

If completion truly requires a forbidden or out-of-scope change, report `NEEDS_REPLAN` instead of making it.

## Requirements

Use stable IDs. Every requirement must be atomic, unambiguous, and testable.

- **R-001 (MUST):** <required behavior or interface>.
- **R-002 (MUST):** <required error handling, edge case, or invariant>.
- **R-003 (MUST NOT):** <forbidden behavior or implementation shortcut>.
- **R-004 (MUST):** <required tests, observability, documentation, or migration behavior>.
- **R-005 (MUST):** Implement the final intended design directly. Do not add a temporary compatibility layer, dual path, deprecated alias, feature flag, or legacy fallback unless this task explicitly requires one.

## Acceptance criteria

Acceptance criteria describe observable behavior, not internal activity such as "create a struct" or "edit a file." Do not mark or rewrite these criteria during execution.

### AC-001 — <primary success scenario>

- **Given:** <starting state or input>.
- **When:** <action or event>.
- **Then:** <specific observable output or state>.

### AC-002 — <important edge or failure scenario>

- **Given:** <starting state or input>.
- **When:** <action or event>.
- **Then:** <specific observable output, error, or preserved invariant>.

### AC-003 — <non-regression or removal scenario>

- **Given:** <existing supported behavior or old implementation>.
- **When:** <the changed path is exercised>.
- **Then:** <required preserved behavior, or proof that the old path is completely removed>.

## Fixed design decisions

The following decisions are already made. The executor must implement them rather than reopening them:

- **D-001:** <chosen architecture/API/library/data model and rationale relevant to execution>.
- **D-002:** <compatibility or migration policy>.
- **D-003:** <required interface, type, function signature, file location, or naming>.

No unresolved architecture, product, security, compatibility, or data-migration decision may remain in a task marked ready for execution.

## Executor discretion

The executor may choose only incidental implementation details that do not change the goal, requirements, acceptance criteria, public contract, checklist structure, or scope. Examples:

- Local helper names and private decomposition consistent with repository conventions.
- The order of task-local edits inside the currently active checklist leaf.
- Equivalent implementation details when the specified path has moved, provided the final behavior and architecture remain unchanged.

Record any non-obvious choice in the progress log and completion report.

## Execution status

This block is the compact, human-visible state of the run. It is the only place in this file that represents an in-progress state. Markdown checkbox syntax remains binary: unchecked means pending and checked means completed.

<!-- goal-task:mutable-status:start -->
- **State:** `NOT_STARTED` <!-- NOT_STARTED | IN_PROGRESS | VERIFYING | DONE | BLOCKED | NEEDS_REPLAN -->
- **Current leaf item:** `NONE` <!-- one unchecked leaf ID while active/blocked; NONE only when NOT_STARTED or DONE -->
- **Leaf progress:** `0/<TOTAL_LEAVES> (0%)`
- **Last checkpoint (UTC):** `—`
<!-- goal-task:mutable-status:end -->

## Execution checklist

### Checklist semantics

- Every checkbox has a unique, stable hierarchical ID.
- An item at depth 1 uses `N`; depth 2 uses `N.N`; depth 3 uses `N.N.N`; depth 4 uses `N.N.N.N`, for example `4.2.7.5`.
- Use exactly four ASCII spaces per nesting level before the `-` marker: 0, 4, 8, or 12 spaces. Do not use tabs.
- Maximum depth is four. Depth must match the number of ID components.
- IDs are immutable during execution. Sibling numbers start at `1`, increase contiguously, and are never reused.
- Parent items are roll-ups. Check a parent only after every descendant is checked and the parent's stated outcome is true.
- A leaf item is an executable checkpoint. It must state what becomes true, reference relevant requirement or acceptance IDs, and say what evidence permits checking it.
- Only leaf items count toward `Leaf progress`. Parent roll-ups are deliberately excluded so nested work is not double-counted.
- Exactly one unchecked leaf may be named as **Current leaf item**. This maps the three execution states without nonstandard checkbox syntax:
  - `[ ]` and not current = `PENDING`.
  - `[ ]` and named as current = `IN_PROGRESS`.
  - `[x]` = `COMPLETED`.
- Set the current leaf before working on it. Check it only after its stated evidence succeeds. For partial work, leave it unchecked and record completed versus remaining work in the progress log.
- Follow numeric depth-first order unless this task explicitly states a different dependency order. Do not skip an eligible earlier leaf merely because a later item is easier.
- The planner generates the complete checklist before dispatch. The executor must not add newly discovered work to it. Record incidental details in the progress log; report `NEEDS_REPLAN` when a missing item changes scope, design, acceptance, or independent completion work.
- Checklist completion is progress evidence, not the completion oracle. `DONE` still requires acceptance evidence and the canonical verifier.

### Checklist

Replace every placeholder before dispatch. Remove unnecessary branches rather than creating artificial depth. Use the fourth level when the work genuinely needs it; the example below demonstrates the required `4.2.7.5`-style address format.

<!-- goal-task:checkbox-state:start -->
- [ ] **1** Establish a verified starting point.
    - [ ] **1.1** Read all required context and applicable repository instructions — complete when the relevant constraints and code flow can be stated accurately in the transcript.
    - [ ] **1.2** Confirm every precondition `P-001..P-NNN` — complete when each is supported by a command, repository state, or named artifact.
    - [ ] **1.3** Run the documented baseline or reproduction — complete when the pre-change result is captured in the progress log.
- [ ] **2** Implement the bounded required behavior.
    - [ ] **2.1** Satisfy <requirement group, such as `R-001` and `R-002`> — complete when its descendant implementation and focused checks pass.
        - [ ] **2.1.1** <Implement one coherent internal unit> (`R-001`) — complete when <focused evidence>.
            - [ ] **2.1.1.1** <Smallest fourth-level implementation or proof checkpoint> (`R-001`, `AC-001`) — complete when `<exact command or inspectable artifact>` shows <expected result>.
        - [ ] **2.1.2** <Implement the next coherent internal unit> (`R-002`) — complete when <focused evidence>.
    - [ ] **2.2** Remove or replace superseded behavior (`R-003`, `R-005`) — complete when the legacy path is absent and relevant regressions remain green.
- [ ] **3** Prove every acceptance criterion.
    - [ ] **3.1** Prove `AC-001` — complete when `<exact command or artifact>` demonstrates the primary success scenario.
    - [ ] **3.2** Prove `AC-002` — complete when `<exact command or artifact>` demonstrates the edge or failure scenario.
    - [ ] **3.3** Prove `AC-003` — complete when `<exact command or artifact>` demonstrates non-regression or complete removal.
- [ ] **4** Complete final review and the trusted gate.
    - [ ] **4.1** Review the final diff against scope, requirements, decisions, and protected paths — complete when unrelated, temporary, generated, and prohibited changes are absent.
    - [ ] **4.2** Run all required focused and regression checks — complete when every mapped command succeeds without skips or weakened coverage.
    - [ ] **4.3** Run `./verify.sh` from `repository_root` — complete when it exits `0` and its final stdout line is exactly `DONE`.
    - [ ] **4.4** Reconcile checklist hierarchy and evidence through item `4.3` — complete when every earlier checked leaf has evidence, every eligible earlier parent is checked, and no earlier item is inconsistent.
    - [ ] **4.5** Prepare the required completion report in the progress log — complete when the draft accurately reflects the final verifier, acceptance evidence, changed paths, checklist state, and contract integrity.
<!-- goal-task:checkbox-state:end -->

## Verification contract

### Canonical gate

Run from `repository_root`:

```sh
./verify.sh
```

The task is complete only when all of the following are simultaneously true:

1. `./verify.sh` exits with code `0`.
2. Its final stdout line is exactly `DONE`.
3. Every acceptance criterion has direct evidence.
4. Every checklist leaf and parent is checked consistently.
5. `Current leaf item` is `NONE`, `State` is `DONE`, and leaf progress is complete.
6. The final diff contains only task-related changes.
7. The task's static contract is unchanged; only allowed status values and checkbox markers differ from the dispatched version.
8. The verifier and all protected paths are unchanged.
9. No task-related failure, skipped check, temporary workaround, placeholder, disabled test, or unresolved TODO remains.

The executor must show the final verifier command, exit code, and terminal result in the transcript. A claim that checks "should pass" is not evidence.

### Task-contract integrity

The trusted outer gate should compare the dispatched task file with the final task file and allow only these state transitions:

- Values between `goal-task:mutable-status:start` and `goal-task:mutable-status:end` may change, but must conform to the status rules.
- Inside `goal-task:checkbox-state:start` and `goal-task:checkbox-state:end`, only the three-character checkbox token may change between `[ ]` and `[x]`.
- No checklist ID, text, requirement reference, completion condition, indentation, ordering, or hierarchy may change.
- No content outside those marked blocks may change.

For deterministic enforcement, normalize the two allowed mutable surfaces before hashing or diffing the contract. Prose alone is not sufficient protection.

### Acceptance-to-evidence map

Every acceptance criterion must map to one or more checklist leaves and at least one executable check or inspectable artifact.

| Criterion | Checklist leaf IDs | Required evidence | Exact command or artifact |
| --- | --- | --- | --- |
| AC-001 | `<for example 2.1.1.1, 3.1>` | <focused test, E2E scenario, API transcript, screenshot, benchmark, etc.> | `<command or path>` |
| AC-002 | `<for example 2.1.2, 3.2>` | <negative or edge-case evidence> | `<command or path>` |
| AC-003 | `<for example 2.2, 3.3>` | <non-regression or removal evidence> | `<command or path>` |

For user-visible behavior, include an end-to-end or direct-use check where feasible; compilation or unit tests alone are not sufficient proof of visible behavior.

## Checklist execution protocol

1. Read repository-level instructions, this entire task, and every source under **Starting point and required context**.
2. State the task ID, one-sentence goal, acceptance-criterion IDs, total leaf count, and first eligible leaf ID in the transcript.
3. Initialize **Execution status**: set `State` to `IN_PROGRESS`, set `Current leaf item` to the first eligible unchecked leaf, and set the correct leaf progress.
4. Follow the checklist in numeric depth-first order. Work on one leaf at a time.
5. Before checking a leaf, run or inspect its stated completion evidence. Record the leaf ID, result, command or artifact, and UTC timestamp in the progress log.
6. Change `[ ]` to `[x]` only after the leaf succeeds. Then check any parent whose descendants and own outcome are all complete.
7. Recalculate leaf progress and select the next eligible unchecked leaf. Keep exactly one current leaf while work remains.
8. After every meaningful verification checkpoint, and before expected context compaction or session stop, update the status block and progress log.
9. Before every `/goal` turn ends, surface the exact compact progress signal defined below in the transcript. The signal must match the task status and checkbox state.
10. On resume, reread this task, identify the current leaf by ID, read its latest evidence in the progress log, inspect `git status` and the relevant diff, and continue from that item rather than reconstructing state from chat memory.
11. When implementation leaves are complete, set `State` to `VERIFYING` and execute the acceptance and final-gate leaves.
12. Do not set `State` to `DONE` or `Current leaf item` to `NONE` until all checklist items, acceptance evidence, scope checks, contract-integrity checks, and the canonical verifier succeed.
13. Produce the exact completion report below. Do not claim `DONE` on any other basis.

### Per-turn progress signal

Before each `/goal` turn ends, print exactly one line in this form:

```text
GOAL_PROGRESS task=TASK-000 state=<STATE> current=<LEAF_ID|NONE> verified_leaves=<DONE>/<TOTAL> newly_completed=<ID[,ID...]|none> blocked=<ID[,ID...]|none> next=<LEAF_ID|NONE>
```

Rules:

- `current` is the one unchecked leaf currently being executed or blocked.
- `verified_leaves` counts checked leaves only; parents are excluded.
- `newly_completed` lists leaves checked during the current turn.
- `next` is the next eligible unchecked leaf in numeric depth-first order, or `NONE` when complete or blocked without another eligible leaf.
- The line is a transcript projection of the authoritative task status and checklist, not a second state store.

## Progress and handoff log

Maintain detailed mutable evidence at the `progress_log` path. The checklist in this task is the authoritative progress structure; the progress log is its evidence and handoff record, not a competing task list.

Create or refresh the log before the first edit, then update it after every checked leaf, any failed checkpoint that changes the next action, and before any expected context compaction or session stop. Keep entries short, factual, and keyed by checklist ID:

```text
TASK: TASK-000
STATE: IN_PROGRESS | VERIFYING | DONE | BLOCKED | NEEDS_REPLAN
CURRENT_ITEM: <hierarchical leaf ID or NONE>
LEAF_PROGRESS: <completed>/<total> (<percent>%)
BASELINE: <command and result>

CHECKPOINTS:
- <UTC timestamp> | <item ID> | COMPLETED | <command/artifact and concise evidence>
- <UTC timestamp> | <item ID> | IN_PROGRESS | completed: <fact>; remaining: <fact>
- <UTC timestamp> | <item ID> | FAILED | <exact failure and next evidence-based action>

CURRENT_FAILURE: <exact failing check or none>
LAST_ACTION: <what changed and what the evidence showed>
NEXT_ACTION: <single best next action, naming its checklist ID>
DECISIONS: <task-local incidental decisions or none>
```

A checked item without corresponding evidence is invalid and must be unchecked. A partially completed item remains unchecked.

## Prohibited shortcuts

The executor must not:

- Edit task prose, checklist text, IDs, hierarchy, ordering, indentation, requirement links, or completion conditions.
- Add, delete, split, merge, or renumber checklist items during execution.
- Use nonstandard checkbox states such as `[-]`, `[~]`, `[>]`, or emoji as the authoritative state.
- Mark an item checked when work merely started, code was merely edited, or its evidence has not succeeded.
- Mark a parent checked while any descendant is unchecked or its parent outcome is false.
- Name a parent as the current item; the active item must be an unchecked leaf.
- Maintain more than one current leaf unless this task explicitly changes `max_active_leaf_items` before dispatch.
- Edit, replace, bypass, chmod, relocate, or otherwise neutralize the verifier, trusted tests, fixtures, snapshots, schemas, policies, or other protected inputs.
- Delete, skip, ignore, quarantine, weaken, or rewrite a failing check merely to obtain a passing result.
- Hard-code implementation behavior only for known test fixtures or verifier inputs.
- Suppress errors, warnings, lint rules, type checks, or failing exit codes.
- Leave the old and new implementations active in parallel unless an explicit requirement demands that architecture.
- Add backward-compatibility aliases, transitional adapters, deprecated commands, or fallback paths that contradict the final intended design.
- Fix unrelated pre-existing failures or perform opportunistic refactors outside the stated boundary.
- Change public behavior, dependencies, schemas, generated artifacts, deployment settings, or external systems unless explicitly required.
- Declare completion from checkbox state, code inspection, partial checks, or a summary without executing the canonical gate.

## Blocked and replan policy

Return **`BLOCKED`** when the task is valid but cannot proceed because of an unmet precondition or an external condition the executor cannot resolve within scope, such as missing credentials, unavailable required infrastructure, or a dependency task that is not complete.

Return **`NEEDS_REPLAN`** when satisfying the task would require changing its goal, acceptance criteria, fixed design decisions, checklist structure, forbidden paths, repository boundary, or primary subsystem; when requirements contradict each other; when a material design decision is still unresolved; or when the generated checklist omits independently trackable work needed for completion.

When blocked or replanning:

- Leave the blocked/current leaf unchecked.
- Set `State` to `BLOCKED` or `NEEDS_REPLAN` and keep `Current leaf item` set to the exact blocked leaf ID.
- Do not check incomplete ancestors.
- Record the blocker under that ID in the progress log.

Do not spin. When no materially new, evidence-supported action remains within the contract, stop with the appropriate status and provide:

- The exact blocker or contradiction.
- The current checklist item ID.
- Commands run and concise evidence.
- Approaches attempted and why they failed.
- The smallest decision, dependency, or task split required to resume.

## Completion report

Use exactly this structure in the final response:

```text
STATUS: DONE | BLOCKED | NEEDS_REPLAN
TASK: TASK-000

SUMMARY:
<What changed or why execution stopped.>

CHECKLIST:
- state: <state>
- current_item: <ID or NONE>
- leaf_progress: <completed>/<total> (<percent>%)
- unchecked_items: <comma-separated IDs or none>
- hierarchy_consistent: YES | NO

ACCEPTANCE:
- AC-001: PASS | FAIL | BLOCKED — <evidence and checklist IDs>
- AC-002: PASS | FAIL | BLOCKED — <evidence and checklist IDs>
- AC-003: PASS | FAIL | BLOCKED — <evidence and checklist IDs>

VERIFY:
- command: ./verify.sh
- exit_code: <integer or NOT_RUN>
- final_stdout_line: <DONE, another line, or NOT_RUN>

CHANGED:
- <repository-relative path> — <reason>

CONTRACT_INTEGRITY:
- static_contract_unchanged: YES | NO
- only_allowed_progress_mutations: YES | NO
- protected_paths_unchanged: YES | NO

DEVIATIONS:
- none
# or list any discovered discrepancy; a material deviation requires NEEDS_REPLAN.

FOLLOW_UP:
- none
# or the smallest required dependency/decision/new task when blocked.
```

<!--
TASK-AUTHOR VALIDATION — remove this comment from instantiated tasks.

A task is READY only when every check below is true:

1. It has exactly one primary outcome and one canonical completion gate.
2. The outcome is independently useful and can be completed in one isolated workspace.
3. It targets one repository and one primary subsystem.
4. It contains no TBDs, open questions, delegated design choices, or hidden dependency on a previous chat.
5. Every dependency and precondition is explicit; blocked tasks are not dispatched.
6. Requirements use stable IDs and MUST/MUST NOT language where appropriate.
7. Acceptance criteria are observable and each maps to concrete evidence and checklist leaf IDs.
8. The verifier already exists, is executable, and can distinguish the current failing state from the required passing state.
9. The verifier and trusted inputs are protected outside the executor's control, not merely protected by prose.
10. Scope and non-goals make unrelated work clearly rejectable.
11. The task requires the final intended implementation, not a soft transition, compatibility shim, or duplicate legacy path unless explicitly chosen.
12. A fresh agent can understand the task from this file, repository instructions, and the named sources alone.

CHECKLIST VALIDATION:

13. Every checkbox has a unique hierarchical numeric ID.
14. ID depth exactly matches indentation: 0/4/8/12 spaces correspond to 1/2/3/4 numeric components.
15. No checklist branch is deeper than four levels.
16. Every non-top-level ID has an existing parent, and sibling numbers are contiguous.
17. Every leaf is an independently checkable checkpoint with a concrete completion condition.
18. Every implementation and proof leaf references applicable `R-*` or `AC-*` IDs.
19. Parent items describe roll-up outcomes rather than duplicate leaf actions.
20. Leaf items are reasonably similar in granularity so the leaf count is a useful progress indicator.
21. The generated checklist normally contains roughly 5–20 leaves. More than 25 leaves is a strong signal to split the task unless the leaves are highly mechanical and uniform.
22. There is no artificial single-child hierarchy merely to reach four levels; use each depth only when it adds meaningful decomposition.
23. The checklist includes baseline/precondition work, implementation, acceptance proof, diff/scope review, canonical verification, and closeout.
24. All checkboxes are unchecked at dispatch, state is `NOT_STARTED`, current leaf is `NONE`, and total leaf count is filled in accurately.
25. The outer gate can enforce the narrow mutation policy by normalizing or diffing marked regions.
26. The executor instructions require the per-turn `GOAL_PROGRESS` transcript line, keyed by the exact current checklist leaf ID.

SPLIT OR REPLAN before dispatch when any of these are true:

- The title or goal contains multiple independently testable outcomes.
- Different acceptance criteria could be completed and shipped independently.
- Work spans multiple repositories or unrelated subsystems.
- The task combines discovery/architecture decisions with production implementation.
- There is no deterministic or inspectable way to decide completion.
- The executor would need to choose product behavior, public API, migration policy, security posture, or compatibility policy.
- Verification depends primarily on "looks good," "clean up," "improve," "fully," "properly," or another undefined judgment.
- The checklist needs more than four meaningful levels or more than about 25 nontrivial leaves.
- A leaf itself contains several independently verifiable outcomes or cannot state one concrete completion condition.
- Completion is likely to require several context compactions or a long internal backlog. Generate a task DAG instead.
-->
```
