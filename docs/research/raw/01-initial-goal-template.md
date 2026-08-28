# Research message 01 — Initial /goal task template research

## Prompt

I found /goal is pretty useful to run long running tasks. But sometimes it's losing context and start doing not what I was requesting. My guess it's because it received a too broad goal with too many tasks to do. Instead of using long running /goal I prefer to use /goal for a small-medium sub task. So the basic idea is to split all tasks and big goal into small portion of tasks and execute those tasks using /goal. That means a new Claude Code or Codex will receive a one task only and it will start with one prompt and fully new context:

/goal Implement this task: @task.md until ./verify.sh will return DONE.

You need to research best practices for using /goal and give me the best structure for such tasks as a template. Our goal to make a reference markdown template file, which will be used to generate these small/medium tasks.

## Response

# Recommended model

Your hypothesis is correct, but the underlying problem is broader than "the goal is too large."

Long `/goal` runs tend to drift when they combine:

* Too many independently completable outcomes.
* Decisions that have not yet been resolved.
* A large amount of unrelated repository context.
* Repeated context compaction.
* Weak or subjective completion criteria.
* Self-verification by the same agent that implemented the change.

Anthropic observed almost exactly this failure pattern: agents attempted too much at once, ran out of useful context, left partially implemented work, and sometimes later declared the broader project complete prematurely. Their most reliable correction was incremental work on one feature at a time, with persistent progress artifacts and explicit verification. ([Anthropic][1])

The correct architecture is therefore:

> **Big goal → planned task DAG → one ready task → one fresh context → one isolated workspace → deterministic verification → independent completion decision.**

OpenAI's current guidance similarly describes a good `/goal` as larger than a one-off prompt but smaller than an open-ended backlog, with one objective, explicit boundaries, a validation loop, and a stopping condition. It explicitly warns against loose lists of unrelated work. ([OpenAI Developers][2])

## The reference template

Store in repositories as:

```text
.agent/GOAL-TASK-TEMPLATE.md
```

Then instantiate tasks as:

```text
tasks/
└── TASK-042/
    ├── task.md          # immutable execution contract
    ├── verify.sh        # immutable trusted verifier
    └── progress.md      # mutable execution state
```

The template includes:

* Machine-readable YAML metadata.
* One-outcome task contract.
* Required context and exact source paths.
* Preconditions and dependencies.
* Explicit in-scope and out-of-scope boundaries.
* Stable requirement IDs.
* Given/When/Then acceptance criteria.
* Fixed design decisions versus executor discretion.
* Acceptance-to-verification traceability.
* A deterministic verifier contract.
* A separate progress/handoff artifact.
* `BLOCKED` and `NEEDS_REPLAN` semantics.
* Prohibited shortcuts.
* A machine-readable completion report.
* A task-author readiness checklist.
* Explicit split/replan conditions.

The reference file is intentionally comprehensive. Generated task instances should remove the author comments and unused placeholders; they should normally be considerably shorter.

# Recommended `/goal` invocation

Replace:

```text
/goal Implement this task: @task.md until ./verify.sh will return DONE.
```

with:

```text
/goal Implement exactly the single immutable task in @task.md.

Before editing, read the task and state its task ID, one-sentence goal,
and acceptance-criterion IDs in the transcript.

Continue until every requirement and acceptance criterion is satisfied,
the final diff is within scope, and ./verify.sh has been run from the
repository root, exits with code 0, and has final stdout line exactly DONE.

Show the final verifier command, exit code, and terminal result in the
transcript. Do not modify task.md, verify.sh, or any protected verifier
input. Do not weaken, skip, delete, or bypass tests or checks. Do not
broaden the task.

If a precondition is false, report BLOCKED with evidence. If completion
requires changing the task boundary, fixed decisions, public contract,
repository, or primary subsystem, report NEEDS_REPLAN with evidence.
Do not reinterpret the task or silently expand its scope.
```

This wording matters especially in Claude Code. Claude's `/goal` evaluator does not independently open files or execute commands; it judges the condition against evidence surfaced in the conversation. Therefore, the agent should state the task identity and acceptance criteria near the beginning and surface the final verifier result near the end. ([Claude][3])

Requiring only the word `DONE` is insufficient. Completion should require the conjunction:

```text
exit code == 0
AND
final stdout line == DONE
AND
protected inputs unchanged
AND
acceptance criteria evidenced
AND
scope review passed
```

# What constitutes one task

Do not define task size by lines of code or number of files. Define it by the **completion surface**.

| Property     | Ready task                                 | Must be split or replanned                                                   |
| ------------ | ------------------------------------------ | ---------------------------------------------------------------------------- |
| Outcome      | One observable end state                   | Several independently useful outcomes                                        |
| Repository   | One repository                             | Cross-repository coordination                                                |
| Architecture | One primary subsystem                      | Unrelated subsystems                                                         |
| Decisions    | Important decisions already made           | Agent must decide product, API, migration, security, or compatibility policy |
| Acceptance   | One coherent set of criteria               | Criteria could pass and ship independently                                   |
| Verification | One canonical gate                         | Several unrelated or subjective notions of "done"                            |
| Dependencies | Explicit and already satisfied             | Hidden prerequisite work                                                     |
| Context      | Understandable from task and named sources | Requires knowledge from an earlier chat                                      |
| Delivery     | One coherent vertical slice                | A backlog, roadmap phase, or collection of cleanup items                     |

GitHub's guidance for autonomous coding agents similarly recommends one logical task per issue, crisp acceptance criteria, and exact links to the relevant files or functions. ([The GitHub Blog][4])

## Split by independently verifiable outcomes

Roadmap: "Migrate the authentication subsystem, update all API clients, remove the legacy protocol, update documentation, add metrics, and deploy it."

Better DAG:

```text
TASK-101  Define and test the new authentication contract
TASK-102  Migrate the authentication server implementation
TASK-103  Migrate the first-party API client
TASK-104  Remove the legacy authentication implementation
TASK-105  Add authentication observability
TASK-106  Update operator and developer documentation
TASK-107  Perform deployment-readiness verification
```

Each task receives a fresh context and sees only its own contract. Dependencies managed by the task tracker or orchestrator. OpenAI's Symphony architecture uses this same pattern: one task maps to one isolated workspace, dependency relationships form a DAG, agents start only on unblocked work. ([OpenAI][5])

## Do not split by technical layer when the behavior is atomic

Do not mechanically create TASK-A: Add database column / TASK-B: Add repository method / TASK-C: Add service method / TASK-D: Add HTTP endpoint / TASK-E: Add endpoint test. That leaves intermediate tasks with no meaningful behavior and no trustworthy completion test.

Prefer a vertical slice: "Expose the account suspension reason through the account API, including persistence, service mapping, response schema, and the contract test proving the behavior."

Key question: Can this task be understood, implemented, verified, reviewed, and accepted as one coherent change?

# The task contract structure

## 1. Static metadata (YAML frontmatter)

```yaml
---
schema: goal-task/v1
id: TASK-042
title: "Reject expired refresh tokens before session rotation"
kind: bugfix
parent: "AUTH-ROADMAP-07"
depends_on:
  - TASK-041
repository_root: "."
base_ref: "main"
subsystem: "authentication/session-rotation"
progress_log: ".goal/progress/TASK-042.md"
verification:
  command: "./verify.sh"
  success_exit_code: 0
  success_last_line: "DONE"
protected_paths:
  - "tasks/TASK-042/task.md"
  - "./verify.sh"
  - "tests/fixtures/refresh-token-contract.json"
---
```

Do not put mutable lifecycle status in this file. `ready`, `running`, `blocked`, `done` live in Linear, GitHub, or orchestrator. Otherwise executor must modify the same document that defines its contract.

## 2. One-sentence goal — describe a state, not list of actions

Good: "Expired refresh tokens are rejected before any session state is rotated."
Bad: "Update token handling, refactor session code, add tests, update docs, clean up old utilities, and improve errors."

## 3. Observable outcome

"When a client submits an expired refresh token, the endpoint returns HTTP 401 with error code `refresh_token_expired`, and no session row, rotation counter, or replacement token is created."

Observable acceptance preferable to internal activity like "add a validation function." OpenAI recommends anchoring long-running tasks in behavior demonstrable through commands, tests, requests, artifacts. ([ChatGPT Learn][6])

## 4. Minimal relevant context

Fresh agent should not require parent conversation or entire roadmap. Include: what is currently wrong; why change needed; how relevant code flow works; exact repository-relative paths; working reproduction or baseline command; references to already-decided design records.

Global repository conventions belong in concise `AGENTS.md` and `CLAUDE.md`. Task-specific facts belong in `task.md`. OpenAI reports giant monolithic instruction file crowds out task and relevant code; use top-level instruction file as map to structured sources of truth. ([OpenAI][7])

## 5. Preconditions

Distinguish "implementation is wrong" from "task dispatched too early."

```markdown
- P-001: TASK-041 is complete and the new token parser is present.
- P-002: The authentication contract fixtures are available locally.
- P-003: The migration decision in `docs/decisions/D-041.md` is final.
```

Agent must return `BLOCKED` when precondition false. Must not silently implement missing dependency.

## 6. Scope and non-goals

Explicit In scope / Out of scope. "Expected touch points" orient without brittle allowlist. "Forbidden touch points" strict.

## 7. Atomic requirements — stable IDs, normative terms

```markdown
- R-001 (MUST): Validate expiration before session rotation begins.
- R-002 (MUST): Return `refresh_token_expired` for an expired token.
- R-003 (MUST NOT): Write any session state on this failure path.
- R-004 (MUST): Preserve existing behavior for valid refresh tokens.
- R-005 (MUST): Remove the superseded validation path rather than retaining both implementations.
```

Stable IDs allow verifier, progress log, completion report, independent reviewer to refer to same contract.

## 8. Behavioral acceptance criteria — Given/When/Then

```markdown
### AC-001 — Expired token is rejected without mutation
- Given: an existing session and an expired refresh token.
- When: the refresh endpoint receives that token.
- Then: it returns HTTP 401 with `refresh_token_expired`, and session state remains byte-for-byte unchanged.
```

Avoid: "The implementation is clean." "The code is fully modern." "Errors are handled properly." "Everything works."

## 9. Fixed decisions and executor discretion

Planner decides architecture, public behavior, compatibility, data migration, security posture, important interfaces before dispatching. Executor may choose incidental details (private helper decomposition, edit order). May not reopen product or architecture decisions.

If important decisions unresolved, generate separate investigation task first:

```text
TASK-041: Determine the refresh-token expiration contract and write docs/decisions/D-041.md.
TASK-042: Implement D-041.
```

Anthropic recommends separating exploration/planning from implementation when approach uncertain. ([Claude][8])

## 10. Acceptance-to-verification mapping

| Criterion | Required evidence | Command or artifact |
| --- | --- | --- |
| AC-001 | Integration test for expired token | `cargo test expired_refresh_token` |
| AC-002 | Valid-token regression test | `cargo test valid_refresh_rotation` |
| AC-003 | No legacy implementation remains | `rg 'legacy_expiry_check' src tests` |

Canonical verifier aggregates these checks.

# Designing `verify.sh`

`verify.sh` is the task's executable definition of done. It should:

1. Use strict shell behavior such as `set -Eeuo pipefail`.
2. Run from a known working directory.
3. Validate protected files or have their integrity validated externally.
4. Run the focused task checks.
5. Run the relevant regression suite.
6. Run formatting, linting, typing, or static analysis relevant to the task.
7. Verify forbidden legacy paths or temporary artifacts are absent.
8. Emit useful failure output.
9. Exit nonzero on every failure.
10. Print `DONE` only as its final line after every check has succeeded.

```bash
#!/usr/bin/env bash
set -Eeuo pipefail

check_protected_inputs
run_focused_behavior_tests
run_relevant_regression_tests
run_static_checks
check_forbidden_legacy_paths
check_scope_and_generated_artifacts

printf 'DONE\n'
```

For visible or end-to-end behavior, do not rely solely on compilation or unit tests. Anthropic found agents often declared features complete after narrow tests even though functionality did not work end to end; explicitly requiring direct-use or browser-level validation substantially improved results. ([Anthropic][1])

## The verifier must be outside the executor's authority

Verifier editable by executor is not trusted. Prose "do not edit `verify.sh`" helps instruction following but not deterministic control. Prefer:

* Mount `task.md` and `verify.sh` read-only.
* Keep authoritative verifier outside writable worktree.
* Sandbox write boundaries excluding task and verifier paths.
* Record expected hashes before launch, validate after agent exits.
* Outer orchestrator runs verifier independently.
* Reject completion if protected-path hashes changed.

Claude Code hooks can deterministically block task completion when tests fail. Hook system supports completion gates and feeds failures back to agent. ([Claude Platform Docs][9])

# Progress state separate from specification

Task contract immutable. Mutable state in `.goal/progress/TASK-042.md`:

```text
TASK: TASK-042
STATUS: IN_PROGRESS
BASELINE: cargo test expired_refresh_token — failed as expected
COMPLETED: R-001, AC-001
CURRENT_FAILURE: valid_refresh_rotation fails with incorrect counter
LAST_ACTION: moved expiration check before transaction creation
NEXT_ACTION: inspect counter initialization for valid tokens
DECISIONS: none
```

Compact handoff for compaction, session recovery, replacement agent, without letting executor rewrite acceptance criteria. Both Anthropic and OpenAI report persistent progress artifacts make long-running work more reliable. ([Anthropic][1])

Claude Code hardening: post-compaction/session-start hook injects instruction to reread task.md, progress.md, git status, git diff, recent task-local commits. Compaction can lose details; supports reinjecting context after compaction. ([Claude Platform Docs][9])

# Completion three layers

## Layer 1: Executor verification — focused checks + canonical verifier while implementing.

## Layer 2: Trusted deterministic gate — outer process confirms:

```text
task hash unchanged
verifier hash unchanged
protected inputs unchanged
verify exit code == 0
verify final stdout line == DONE
```

Task tracker must not transition to `done` before this passes.

## Layer 3: Fresh semantic reviewer — for security-sensitive, architectural, migration, high-consequence work. Fresh reviewer context with only: immutable task, final diff, verification output, relevant repository instructions. Reviewer does not modify code initially; determines whether implementation satisfies every requirement and whether verifier has blind spots.

Anthropic's long-running harness research found separating generator from skeptical evaluator strong reliability improvement, including for tasks with executable checks. ([Anthropic][10])

# Task readiness gate

Task must not be dispatched until:

1. Exactly one primary outcome.
2. One repository and one primary subsystem.
3. All prerequisite tasks complete.
4. No unresolved decisions or placeholders.
5. Does not depend on prior chat memory.
6. Requirements and acceptance criteria have stable IDs.
7. Every acceptance criterion maps to evidence.
8. Canonical verifier already exists.
9. Verifier can distinguish current state from desired state.
10. Verifier inputs protected outside executor's authority.
11. Scope and non-goals explicit.
12. Task describes final intended design, not unspecified soft transition.
13. `BLOCKED` and `NEEDS_REPLAN` have precise meanings.
14. Fresh agent can begin from repository, task file, named sources alone.

Reject and split when: combines research with production implementation; spans independent outcomes; lacks trustworthy completion oracle; likely requires several context compactions and internal backlog.

# Final recommended operating loop

```text
1. Planner researches and decides the larger design.
2. Planner creates a dependency DAG of atomic vertical tasks.
3. A task linter rejects incomplete or overly broad task contracts.
4. The orchestrator selects one unblocked task.
5. It creates a fresh worktree and fresh Claude Code or Codex context.
6. The executor receives only task.md plus normal repository instructions.
7. /goal iterates against the task-local verifier.
8. Progress is persisted outside task.md.
9. The trusted outer gate reruns verification and checks protected hashes.
10. A fresh reviewer audits semantic correctness where required.
11. Only the orchestrator marks the task done.
12. Newly unblocked dependent tasks become eligible.
```

## Sources

[1]: https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents
[2]: https://developers.openai.com/cookbook/examples/codex/using_goals_in_codex
[3]: https://code.claude.com/docs/en/goal
[4]: https://github.blog/developer-skills/github/less-todo-more-done-the-difference-between-coding-agent-and-agent-mode-in-github-copilot/
[5]: https://openai.com/index/open-source-codex-orchestration-symphony/
[6]: https://learn.chatgpt.com/cookbook/articles/codex_exec_plans
[7]: https://openai.com/index/harness-engineering/
[8]: https://code.claude.com/docs/en/best-practices
[9]: https://docs.anthropic.com/en/docs/claude-code/hooks-guide
[10]: https://www.anthropic.com/engineering/harness-design-long-running-apps

---

## Reference file: GOAL-TASK-TEMPLATE.md (v1, from message 01)

```markdown
---
schema: goal-task/v1
id: TASK-000
title: "<imperative title describing one observable outcome>"
kind: implementation # implementation | bugfix | refactor | migration | test | docs | investigation
parent: "<roadmap, issue, decision record, or plan reference>"
depends_on: [] # task IDs that must already be complete
repository_root: "."
base_ref: "<expected branch or commit, when relevant>"
subsystem: "<one primary subsystem>"
progress_log: ".goal/progress/TASK-000.md"
verification:
  command: "./verify.sh"
  success_exit_code: 0
  success_last_line: "DONE"
protected_paths:
  - "<repository-relative path to this task file>"
  - "./verify.sh"
  # Add trusted tests, fixtures, snapshots, schemas, or policies the executor must not weaken.
---

# TASK-000 — <imperative title describing one observable outcome>

> **Immutable execution contract**
>
> This file is the source of truth for one `/goal` run. The executor MUST NOT edit this task, the canonical verifier, or any protected path. Lifecycle status belongs in the task tracker, not in this file. If the contract is contradictory, incomplete, or cannot be satisfied within its boundaries, report `BLOCKED` or `NEEDS_REPLAN`; do not reinterpret or broaden it.

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

- This task file.
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

The executor may choose only incidental implementation details that do not change the goal, requirements, acceptance criteria, public contract, or scope. Examples:

- Local helper names and private decomposition consistent with repository conventions.
- The order of task-local edits and focused test runs.
- Equivalent implementation details when the specified path has moved, provided the final behavior and architecture remain unchanged.

Record any non-obvious choice in the completion report.

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
4. The final diff contains only task-related changes.
5. This task, the verifier, and all protected paths are unchanged.
6. No task-related failure, skipped check, temporary workaround, placeholder, disabled test, or unresolved TODO remains.

The executor must show the final verifier command, exit code, and terminal result in the transcript. A claim that checks "should pass" is not evidence.

### Acceptance-to-evidence map

Every acceptance criterion must map to at least one executable check or inspectable artifact.

| Criterion | Required evidence | Exact command or artifact |
| --- | --- | --- |
| AC-001 | <focused test, E2E scenario, API transcript, screenshot, benchmark, etc.> | `<command or path>` |
| AC-002 | <negative or edge-case evidence> | `<command or path>` |
| AC-003 | <non-regression or removal evidence> | `<command or path>` |

For user-visible behavior, include an end-to-end or direct-use check where feasible; compilation or unit tests alone are not sufficient proof of visible behavior.

## Required execution sequence

1. Read repository-level instructions and every source under **Starting point and required context**.
2. State the task ID, one-sentence goal, and acceptance-criterion IDs in the transcript so the `/goal` evaluator can track the contract.
3. Confirm all preconditions and reproduce or record the baseline before editing.
4. Inspect the relevant implementation and tests; do not scan unrelated parts of the repository without a task-specific reason.
5. Implement the smallest coherent change that satisfies the complete task. Preserve a working state between meaningful steps.
6. Add or update tests that prove the changed behavior without weakening existing coverage.
7. Run the smallest relevant checks after each focused change. Use each failure to choose one evidence-based next correction rather than making several speculative changes at once.
8. Review `git diff` against the goal, requirements, scope boundaries, and protected paths. Remove unrelated or temporary changes.
9. Run the canonical verifier from the required working directory until it succeeds.
10. Produce the exact completion report below. Do not claim `DONE` on any other basis.

## Progress and handoff

Maintain the mutable progress log at the `progress_log` path. Do not put progress state into this immutable task file.

Create or refresh the log before the first edit, then update it after every meaningful verification checkpoint and before any expected context compaction or session stop. Keep it short and factual:

```text
TASK: TASK-000
STATUS: IN_PROGRESS | VERIFYING | DONE | BLOCKED | NEEDS_REPLAN
BASELINE: <command and result>
COMPLETED: <requirement/acceptance IDs with evidence>
CURRENT_FAILURE: <exact failing check or none>
LAST_ACTION: <what changed and what the evidence showed>
NEXT_ACTION: <single best next step>
DECISIONS: <task-local decisions or none>
```

On resume or restart, read this task, the progress log, `git status`, the relevant diff, and recent task-local commits before making new changes. Treat the log as a handoff aid, never as authority to alter the task contract.

## Prohibited shortcuts

The executor must not:

- Edit, replace, bypass, chmod, relocate, or otherwise neutralize the task, verifier, trusted tests, fixtures, snapshots, schemas, policies, or other protected inputs.
- Delete, skip, ignore, quarantine, weaken, or rewrite a failing check merely to obtain a passing result.
- Hard-code implementation behavior only for known test fixtures or verifier inputs.
- Suppress errors, warnings, lint rules, type checks, or failing exit codes.
- Leave the old and new implementations active in parallel unless an explicit requirement demands that architecture.
- Add backward-compatibility aliases, transitional adapters, deprecated commands, or fallback paths that contradict the final intended design.
- Fix unrelated pre-existing failures or perform opportunistic refactors outside the stated boundary.
- Change public behavior, dependencies, schemas, generated artifacts, deployment settings, or external systems unless explicitly required.
- Declare completion from code inspection, partial checks, or a summary without executing the canonical gate.

## Blocked and replan policy

Return **`BLOCKED`** when the task is valid but cannot proceed because of an unmet precondition or an external condition the executor cannot resolve within scope, such as missing credentials, unavailable required infrastructure, or a dependency task that is not complete.

Return **`NEEDS_REPLAN`** when satisfying the task would require changing its goal, acceptance criteria, fixed design decisions, forbidden paths, repository boundary, or primary subsystem; when requirements contradict each other; or when a material design decision is still unresolved.

Do not spin. When no materially new, evidence-supported action remains within the contract, stop with the appropriate status and provide:

- The exact blocker or contradiction.
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

ACCEPTANCE:
- AC-001: PASS | FAIL | BLOCKED — <evidence>
- AC-002: PASS | FAIL | BLOCKED — <evidence>
- AC-003: PASS | FAIL | BLOCKED — <evidence>

VERIFY:
- command: ./verify.sh
- exit_code: <integer or NOT_RUN>
- final_stdout_line: <DONE, another line, or NOT_RUN>

CHANGED:
- <repository-relative path> — <reason>

PROTECTED_PATHS:
- unchanged: YES | NO

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
7. Acceptance criteria are observable and each maps to concrete evidence.
8. The verifier already exists, is executable, and can distinguish the current failing state from the required passing state.
9. The verifier and trusted inputs are protected outside the executor's control, not merely protected by prose.
10. Scope and non-goals make unrelated work clearly rejectable.
11. The task requires the final intended implementation, not a soft transition, compatibility shim, or duplicate legacy path unless explicitly chosen.
12. A fresh agent can understand the task from this file, repository instructions, and the named sources alone.

SPLIT OR REPLAN before dispatch when any of these are true:

- The title or goal contains multiple independently testable outcomes.
- Different acceptance criteria could be completed and shipped independently.
- Work spans multiple repositories or unrelated subsystems.
- The task combines discovery/architecture decisions with production implementation.
- There is no deterministic or inspectable way to decide completion.
- The executor would need to choose product behavior, public API, migration policy, security posture, or compatibility policy.
- Verification depends primarily on "looks good," "clean up," "improve," "fully," "properly," or another undefined judgment.
- Completion is likely to require several context compactions or a long internal backlog. Generate a task DAG instead.
-->
```
