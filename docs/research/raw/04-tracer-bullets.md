# Research message 04 — Tracer Bullets + AIHero `/implement` vs `task-format`

Date: 2026-08-28. Snapshot analysed by the research: `main` as of the v3 package +
container harness (pre-`d91e6c0`, pre-`55d1b72`). Read `notes/tracer-bullets.md`
for the delta against HEAD; this file is the raw input, stored verbatim.

## Prompt

Research this project: https://github.com/donbeave/task-format

We are looking to create the best practical task format to most effectively use /goal commands in AI coding agents, like Claude Code, Codex, Grok Build, etc...

Research these links:
https://www.aihero.dev/tracer-bullets
https://www.aihero.dev/skills-implement

And specifically Tracer Bullets concept to understand where this can fit into our task-format project to improve the current task format to make AI be predictable as much as we can.

## Response

# Research conclusion

**Tracer Bullets should become a first-class structural invariant in `task-format`, not merely another paragraph in `AGENTS.md` and not merely a label on a checklist item.**

The target rule should be:

> **One executable task package contains one primary, production-quality, end-to-end tracer bullet; the agent must make its trusted verification seam green before implementing secondary cases or broadening the change. A second independent behavior becomes a second task and a fresh context.**

This fits the central idea in both sources: agents tend to construct complete horizontal layers before validating whether the path works; a tracer bullet instead proves one small path through the relevant layers, obtains feedback immediately, and only then expands. AIHero also recommends a fresh context between independent slices. ([AI Hero][1]) The canonical Pragmatic Programmer guidance reinforces the same pattern: find the target through feedback, take small steps, prove assumptions in the real environment, and build small pieces end-to-end rather than top-down or bottom-up. ([Pragmatic Bookshelf][2])

The current v3 architecture is already strong around the **outer execution boundary**. Its main weakness is the **order in which work becomes verified inside that boundary**.

---

# 1. What Tracer Bullets mean for `/goal`

A tracer bullet is not simply:

* a small task;
* the first checkbox;
* a prototype;
* one backend module;
* one test file;
* or one implementation layer.

It is a **small but complete observable path**:

```text
trigger
  → public entry point
  → relevant domain logic
  → adapter or persistence boundary
  → observable result
  → trusted verification
```

For example:

```text
User selects "Reveal in File System"
  → one context-menu entry
  → backend endpoint
  → platform command
  → file manager opens
  → integration test or observed command result
```

It deliberately does **not** start by implementing every menu location, every API variation, all error handling, and all supporting abstractions. AIHero's example similarly connects one backend endpoint to one UI location before expanding to the other locations. ([AI Hero][1])

## Two different levels are involved

`task-format` should apply Tracer Bullets at two levels.

### Level A — task decomposition

The upstream planner or task compiler turns a large feature into multiple independently verifiable task packages:

```text
approved specification
  → tracer task 1
  → tracer task 2
  → tracer task 3
  → hardening task
```

Each task gets:

* one fresh context;
* one isolated workspace;
* one primary behavior;
* one trusted gate;
* one completion decision.

This is very close to the existing project architecture. Its own research already describes:

```text
Big goal
  → planned task DAG
  → one ready task
  → one fresh context
  → isolated workspace
  → deterministic verification
```

The example-application decomposition also correctly rejects implementing the entire application as one task because it contains multiple user-visible behaviors, too many leaves, and no useful incremental gate.

### Level B — execution order inside one task

Even a correctly bounded task can still be executed horizontally:

```text
build database layer
build domain layer
build UI layer
build CLI layer
finally run integration tests
```

Instead, the executor should follow:

```text
reproduce primary failure
  → make the thinnest complete path pass
  → preserve that passing path
  → add error cases and variants
  → run integrated gate
```

This is the missing piece in v3.

The `/implement` workflow describes this as TDD at pre-agreed seams, one red-green slice at a time, with focused tests during implementation and the full suite at the end. It also says one run should cover one tracer-bullet ticket in one fresh context. ([AI Hero][3]) The underlying TDD skill explicitly identifies "all tests first, then all implementation" as horizontal slicing and instead prescribes one test followed by one minimal implementation repeatedly.

---

# 2. Important distinction: tracer bullet versus prototype

One part of the AIHero article should **not** be imported literally into `task-format`: it associates tracer bullets with `/prototype` and throwaway code.

The canonical Pragmatic Programmer material lists these as two separate techniques:

* tracer bullets find the target through working feedback;
* prototypes are created to learn, with value in the lessons rather than the resulting code. ([Pragmatic Bookshelf][2])

For this project, the practical distinction should be:

| Technique                    | Where it belongs                                                             |
| ---------------------------- | ---------------------------------------------------------------------------- |
| **Prototype**                | Upstream planning or research, before fixed decisions are frozen             |
| **Tracer bullet**            | The executable task itself; minimal but production-quality code that remains |
| **Hardening**                | Later criteria within the same behavior, or subsequent tasks                 |
| **Independent new behavior** | A new task and fresh context                                                 |

That preserves the current template's direct-final-design rule: no temporary compatibility layer, speculative fallback, or legacy path unless the contract explicitly requires one.

If a tracer exposes a false architectural assumption, the executor should not turn the task into an experiment. It should stop with `NEEDS_REPLAN`, provide the evidence, and let the planner revise the task.

---

# 3. Assessment of the current `task-format`

## What v3 already gets right

The current package has several features that are stronger than `/implement` itself:

1. **Immutable task contract.** `README.md`, protocol, and verifier are read-only.
2. **Separate mutable progress state.** Only checkbox tokens and execution evidence change.
3. **Exact precondition commands.**
4. **Requirements, acceptance criteria, and fixed decisions with stable IDs.**
5. **Exact evidence commands and expected results.**
6. **A path whitelist enforced by the host gate.**
7. **Explicit `BLOCKED` and `NEEDS_REPLAN` outcomes.**
8. **The host verifier, not the agent's claim, decides completion.**
9. **A reference solution must prove that the gate can distinguish baseline from success.**

These are all valuable and should remain.

The actual `/implement` skill is much thinner. Its complete operating instruction is essentially: implement the ticket, use TDD where possible at pre-agreed seams, typecheck and run focused tests, run the suite, review, and commit.  The AIHero documentation itself identifies important weaknesses: seams are not actually agreed by the skill, the ticket remains open, acceptance boxes are not reconciled, review findings are not necessarily acted upon, and the skill trusts whatever task shape it receives. ([AI Hero][3])

`task-format` should therefore **borrow the execution rhythm from `/implement`, not replace its stronger contract and verifier with `/implement`.**

## The current gap

The v3 template mentions a "vertical slice" in Scope, but this is only advisory prose. The linter verifies section order, IDs, commands, checklist depth, evidence, acceptance coverage, scope configuration, and final gate placement; it does not verify that the task or its first implementation checkpoint is vertical.

The execution protocol then says to work checklist leaves in numeric order. That makes the quality of the execution sequence entirely dependent on how the author happened to organize the checklist.

This permits the exact behavior Tracer Bullets are intended to prevent.

---

# 4. `TASK-101` demonstrates the problem

`TASK-101` has a genuinely end-to-end goal:

> Launch `pgtui`, load saved connections from Turso, display the list, and quit cleanly.

But its implementation checklist is organized horizontally:

1. implement the Turso store;
2. implement app state and keys;
3. implement rendering;
4. implement the CLI;
5. add unit tests;
6. later rerun the acceptance criteria.

Its acceptance commands separately test:

* store behavior;
* snapshots;
* app state;
* key handling;
* CLI error handling.

There is no single primary command that proves:

```text
seeded store
  → real application startup
  → stored row loaded
  → rendered first screen
  → q processed
  → clean exit
```

Therefore the task can accumulate substantial code before any one command proves that its actual stated user path works.

It also includes behavior that belongs to a later vertical slice:

* `insert`;
* duplicate-name handling;
* `created_at` creation behavior.

Those are needed by `TASK-102`, where the user creates a connection. They are not necessary to prove the first "list saved connections" tracer.

The decomposition note itself carries the same layer-oriented checklist, which shows that macro-level task splitting was considered, but the tracer ordering was lost when the plan was compiled into the executable task.

## Better shape for `TASK-101`

The primary tracer should be something like:

> Given a store containing one saved connection, launching `pgtui --db <path>` renders that connection on the first screen, and pressing `q` exits successfully with the terminal restored.

Its primary trusted command could be:

```sh
cargo test -p pgtui --test tracer_connection_list_test
```

That one test should drive the highest practical stable seam and exercise:

```text
CLI
  → store open/list
  → app initialization
  → first render
  → key processing
  → terminal teardown
```

Only after that is green should the task broaden to:

* empty state;
* sorting;
* `j`/`k` clamping;
* unwritable database error;
* `Ctrl+C`.

Creation and duplicate-name behavior should remain in `TASK-102`.

---

# 5. `TASK-106` should be split

The current `TASK-106` combines:

1. disconnect and return to the list;
2. universal quit and terminal restoration;
3. generation of SVG and PNG gallery artifacts.

These have different:

* triggers;
* outcomes;
* implementation paths;
* verification surfaces;
* failure modes.

They are three separate tracer or atomic tasks:

```text
TASK-A — Disconnect returns to ConnectionList
TASK-B — Exit always restores the terminal
TASK-C — Gallery produces the required screen artifacts
```

Combining them gives the agent several independently completable finish lines, which weakens `/goal` completion judgment and makes progress percentages less meaningful.

---

# 6. Proposed task-format change

I would introduce an experimental schema first:

```yaml
schema: task/v3-tracer
execution_shape: tracer-bullet
```

After comparative runs demonstrate improvement, promote it to `task/v4`.

The supported shapes should initially be limited to:

```text
tracer-bullet   # default for feature and bugfix
atomic-change   # narrow refactor, removal, docs, mechanical test change
```

Avoid introducing many execution modes before experiments demonstrate a need for them.

A wide migration can be decomposed into explicitly authorized phases. The upstream `to-tickets` skill recommends expand–contract for wide refactors that cannot remain green as a vertical slice.  In this project, that should remain an **explicit planner decision**, not something the executor may choose spontaneously, because the default policy correctly prohibits dual paths and compatibility layers.

---

# 7. Add a required `Tracer bullet` section

Place it after `Context` and before `Preconditions`.

```markdown
## Tracer bullet

Execution shape: `tracer-bullet`.

Primary user path:

`<trigger>` → `<public entry point>` → `<relevant system path>` → `<observable result>`

Primary verification seam:

- Boundary: <public interface at which behavior is observed>.
- Primary criterion: `AC-001`.
- Evidence: `<exact command>`.
- Expected baseline: `<specific RED result>`.
- Expected after implementation: `<specific GREEN result>`.

Learning boundary:

- Make `AC-001` green before implementing `AC-002+`.
- The tracer is production-quality code, not throwaway scaffolding.
- Do not implement adjacent entry points, variants, or future capabilities before
  the primary path is green.

Plan-invalidating observations:

- <observation that proves the declared seam or architecture is wrong>
  → stop `NEEDS_REPLAN`.
- <required path outside scope>
  → stop `NEEDS_REPLAN`.
```

This section does four things that the current template does not:

1. identifies the one path that must work first;
2. names the public test seam before execution;
3. defines the first machine feedback checkpoint;
4. defines what evidence invalidates the plan.

## Reserve `AC-001` for the primary tracer

The acceptance table should add a seam column:

```markdown
| ID | Verification seam | Given / When / Then | Evidence command | Expected |
| --- | --- | --- | --- | --- |
| AC-001 | <highest stable public seam> | <primary end-to-end behavior> | `<primary command>` | `<green result>` |
| AC-002 | <same capability, error path> | <negative behavior> | `<command>` | `<result>` |
| AC-003 | <regression seam> | <preserved invariant> | `<command>` | `<result>` |
```

Rules:

* `AC-001` is always the primary tracer for `execution_shape: tracer-bullet`.
* The baseline command must be the `AC-001` evidence command, or a mechanically equivalent filtered form.
* `AC-002+` may harden the same capability.
* A criterion with a different actor, entry point, or terminal outcome normally belongs to another task.

This gives every task a stable semantic center.

---

# 8. Replace the current implementation-first checklist

The present skeleton is:

```text
baseline
implementation units
acceptance proofs
gate
```

The improved skeleton should be:

```markdown
<!-- checklist:start -->
- [ ] **1** Primary target is established.
    - [ ] **1.1** Preconditions `P-001..P-NNN` pass — evidence: each command exits 0.
    - [ ] **1.2** `AC-001` is RED for the expected reason and the observation is recorded in `BASELINE:` — evidence: `<primary command>` → `<expected baseline failure>`.

- [ ] **2** Primary tracer is GREEN (`AC-001`).
    - [ ] **2.1** The smallest production-quality end-to-end path works — evidence: `<primary command>` → `<expected success>`.
    - [ ] **2.2** No adjacent capability or speculative path was added — evidence: diff contains only code required by `AC-001`, its fixed decisions, and necessary support.

- [ ] **3** The same capability is hardened.
    - [ ] **3.1** `AC-002` <negative or edge behavior> — evidence: `<command>` → `<expected>`.
    - [ ] **3.2** `AC-003` <preserved invariant or superseded path removal> — evidence: `<command>` → `<expected>`.

- [ ] **4** Integrated completion is proven.
    - [ ] **4.1** All acceptance commands pass together and `AC-001` remains green — evidence: `<combined focused command>` exits 0.
    - [ ] **4.2** Only `expected_paths` changed and no temporary work remains — evidence: `git status --porcelain` and `git diff --stat`.
    - [ ] **4.3** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full output is shown in the transcript.
<!-- checklist:end -->
```

## Why this is better

The first meaningful milestone is no longer:

> the store layer is complete.

It becomes:

> the declared user path works through its agreed public seam.

The current checklist also frequently repeats the same acceptance commands once under "implementation" and again under "acceptance criteria are proven." That creates duplicated state and delays the psychological finish line. Under the tracer structure:

* each criterion has one owning leaf;
* later integrated proof reruns everything together;
* the host gate remains the final authority.

The checklist still reports progress clearly, but it reports **working capability**, not completed layers.

---

# 9. Add one short protocol invariant

Do not paste the full Tracer Bullets article into `AGENTS.md`. Long advisory prose will be weaker than a small enforceable rule.

Add something close to:

```markdown
## Primary feedback loop

For `execution_shape: tracer-bullet`, `AC-001` is the primary tracer.

After reproducing its declared baseline failure, work only on the smallest
production-quality path needed to make the exact `AC-001` command pass.
Do not begin `AC-002+`, broad cleanup, adjacent variants, or future capabilities
before `AC-001` is green.

After `AC-001` becomes green, preserve it after every later checklist leaf.
If the declared seam, required path, fixed decision, or expected baseline is
materially wrong, stop with `NEEDS_REPLAN`; do not redesign the task.
```

This supplies the missing **iteration policy**.

That term is especially important because current OpenAI Goal guidance says a strong goal should define the outcome, verification surface, constraints, boundaries, iteration policy, and blocked stop condition. The existing task format already covers five of those well; Tracer Bullets provide the missing iteration policy. ([OpenAI Developers][4])

---

# 10. Strengthen the verifier around one primary command

Add to `verify.config`:

```bash
PRIMARY_CMD="<trusted AC-001 command>"
```

`verify.sh` should run checks in this order:

```text
scope/config integrity
primary tracer
focused hardening checks
regression checks
lint
progress consistency
DONE
```

Output:

```text
CHECK primary PASS
CHECK focused-1 PASS
CHECK regression-1 PASS
CHECK lint-1 PASS
...
DONE
```

For `tracer-bullet` tasks, dispatch should require:

```text
PRIMARY_CMD fails on untouched baseline for the declared reason
PRIMARY_CMD passes with the reference solution
```

The existing project already requires the overall gate to fail on baseline and pass with a reference solution. The improvement is making sure the failure/pass pair includes the **primary end-to-end path**, rather than only a collection of component tests.

## Trusted seam requirement

Where practical, the primary test should be planner-supplied and outside `expected_paths`, so the executor cannot weaken it.

A primary seam should be:

* HTTP or RPC boundary;
* CLI invocation;
* public library API;
* application state transition plus renderer;
* persistent-store integration;
* generated artifact inspection;
* benchmark or deterministic script.

An internal grep can be a useful structural guard, but it should not be the primary proof of user behavior.

For example, in the expired-token example, the HTTP refresh behavior should be primary. Checking that `validate` appears before `begin_transaction` is an implementation guard, not the main seam.

---

# 11. Extend `task-lint.sh`, but do not pretend syntax can prove verticality

## Deterministic checks that should fail lint

For `execution_shape: tracer-bullet`:

1. `## Tracer bullet` exists in the required position.
2. `AC-001` exists.
3. The section identifies a public verification seam.
4. It contains RED and GREEN expected results.
5. The baseline command equals the `AC-001` evidence command.
6. `PRIMARY_CMD` equals the `AC-001` evidence command.
7. The first implementation leaf after baseline references `AC-001`.
8. No `AC-002+` leaf appears before the primary-green leaf.
9. Every AC has exactly one owning checklist leaf.
10. The final integrated leaf reruns or transitively includes `AC-001`.
11. Feature and bugfix tasks use `tracer-bullet` unless a specific exception is declared.
12. `atomic-change` tasks have a primary invariant and postcondition instead.

## Warnings rather than hard failures

The linter can warn when:

* the title joins several outcomes with "and";
* the Goal contains multiple independent verbs or artifacts;
* implementation parents are named after layers such as "database," "backend," "frontend," "UI," or "CLI";
* `expected_paths` spans many unrelated subsystems;
* no one evidence command appears to touch the public entry point;
* requirements mention behavior not reachable through the primary path.

These are useful heuristics, not proofs.

## Semantic author review

True verticality requires judgment. Before dispatch, the planner or an independent task reviewer should answer:

```text
1. Is AC-001 independently observable?
2. Does its command exercise the highest practical public seam?
3. Does it cross every layer necessary for the stated behavior?
4. Can it become green before secondary cases are implemented?
5. Does every remaining requirement harden the same capability?
6. Is any requirement actually preparation for a later task?
7. Would two competent reviewers agree on pass/fail?
8. Does the task fit one fresh context?
```

A model-based review can help, but it should not replace the deterministic linter or the trusted verifier.

---

# 12. What to borrow from `/implement`

## Borrow

The useful concepts are:

* the plan is already decided before execution;
* one ticket per run;
* one fresh context per ticket;
* pre-agreed public seams;
* one red-green slice at a time;
* run focused tests repeatedly;
* typecheck regularly;
* run the broad suite near the end;
* do not silently reopen architecture. ([AI Hero][3])

The accompanying `to-tickets` skill is also directionally correct: each ticket should be a narrow but complete path through all relevant layers, independently verifiable and sized for one fresh context.

## Do not borrow

Do not rely on:

* the executor discovering the seams during the run;
* an extremely terse skill prompt as the only control;
* same-agent code review as independent verification;
* committing as the definition of completion;
* unchecked tracker state;
* one shared checkout for concurrent agents;
* accepting a horizontal ticket and hoping `/implement` reshapes it.

The `/implement` documentation explicitly says it trusts upstream task shaping and will build a horizontally structured ticket as written. ([AI Hero][3])

`task-format` should therefore serve as the stronger **compiled input contract** that `/implement` is missing.

---

# 13. Adapt "get feedback" for unattended `/goal`

AIHero's human workflow says:

```text
build slice
test
get feedback
move to next slice
```

An unattended `/goal` run cannot depend on a human replying after the first green test. Its feedback mechanism should be:

```text
trusted command output
  → compare to declared expectation
  → continue with same-slice hardening
  OR
  → NEEDS_REPLAN
```

Human or planner feedback occurs:

* before dispatch, when the tracer is selected;
* between task packages;
* after `NEEDS_REPLAN`;
* during independent review after completion.

This preserves one-shot execution without allowing the agent to build blindly.

---

# 14. Cross-agent implications

## Claude Code

Claude's `/goal` evaluator checks what the agent surfaces in the conversation; it does not independently run commands or inspect files. A useful condition therefore needs one measurable end state, a stated check, important constraints, and a bound. ([Claude Platform Docs][5])

Consequences:

* full `PRIMARY_CMD` output must appear in the transcript;
* final verifier output must appear in the transcript;
* the existing `GOAL_PROGRESS` signal remains useful;
* no extra mutable `PHASE:` field is needed;
* the primary-green milestone can be derived from the completion of its fixed checklist leaf.

## Codex

The repository's Codex documentation needs updating. Current official Codex supports persistent `/goal`; Goals are available starting with Codex 0.128.0, can be paused and resumed, and are explicitly designed around evidence-backed continuation. ([OpenAI Developers][4])

The repository's headed launcher already:

* enables `[features] goals = true`;
* launches the interactive Codex TUI;
* injects the common first `/goal` text block.

But `goal-prompt.md` still describes Codex primarily as `codex exec` with retry/resume rather than native headed `/goal`.

The correction should be:

```text
headed Codex TUI:
  use native /goal

headless codex exec:
  retain a separate fallback only if that surface cannot activate a Goal
```

Also update status detection to distinguish:

* native goal active;
* native goal complete;
* budget-limited;
* blocked;
* ordinary agent idle.

## Grok Build

Grok Build's `/goal` continues until completion and verification, automatically plans an approach, and creates its own progress checklist. ([SpaceXAI][6])

That introduces a possible duplicate source of truth:

```text
immutable README checklist
mutable progress.md checklist
Grok native goal checklist
```

The Grok adapter should explicitly state:

> The checklist in `/task/README.md`, mirrored in `/progress/progress.md`, is authoritative. Any native goal checklist is a UI projection only and must correspond to the existing task leaves; it may not redefine, add, remove, or reorder task scope.

Completion still depends only on the trusted host verifier, not on Grok's panel showing every native checkbox as complete.

The current launcher only accepts `claude|codex`, so Grok support should be added as an adapter and image without changing the universal task schema.

---

# 15. Recommended experiment before promoting v4

The repository has built and self-tested the v3 package and container harness, but its first headed Claude and Codex execution runs are still listed as the next step. Therefore neither v3 nor the tracer revision has yet been empirically shown to be "best."

Do not overwrite v3 immediately. Run two separate ablations.

## Experiment A — execution-order ablation

Keep the same task scope and tests.

```text
Variant H:
  current layer-oriented checklist

Variant T:
  tracer section
  AC-001 primary command
  RED → GREEN first
  hardening afterward
```

This isolates the effect of tracer-first execution order.

Use at least:

* one cross-layer feature such as `TASK-101`;
* one bugfix such as the expired-token example;
* one narrow refactor or removal as an atomic control.

## Experiment B — task-boundary ablation

Compare:

```text
one broad task containing several outcomes
```

against:

```text
several one-tracer tasks with fresh contexts
```

This measures macro decomposition separately from checklist ordering.

## Metrics that matter

The main metric should not merely be tokens or elapsed time. Predictability means lower variance and fewer late surprises.

Capture:

| Metric                                         | What it reveals                        |
| ---------------------------------------------- | -------------------------------------- |
| Host-gate first-pass rate                      | Actual completion reliability          |
| Native `/goal` complete while host gate fails  | False completion rate                  |
| Turns/tool calls until primary green           | Feedback-loop length                   |
| Files and lines changed before primary green   | Amount of "dark work"                  |
| Lines rewritten or deleted after primary green | Rework and architectural miss          |
| Scope violations                               | Contract adherence                     |
| `NEEDS_REPLAN` rate                            | Task-authoring defects surfaced safely |
| Context compactions                            | Task/context fit                       |
| Total tokens and cost                          | Execution efficiency                   |
| Variance across repeated runs                  | Predictability                         |
| Total cost for the whole multi-task feature    | Whether context resets are worth it    |

The most directly tracer-specific metric is:

```text
unverified work before first end-to-end green
```

The tracer variant should materially reduce that quantity.

---

# 16. Recommended repository changes

In implementation order:

1. **Record a new research decision** in `RESEARCH-FINDINGS.md`:

   ```text
   D24 — Feature and bugfix packages contain one primary tracer bullet.
   AC-001 is its pre-agreed public verification seam.
   The executor must establish RED and make AC-001 GREEN before later criteria.
   Independent behavior is a new task and fresh context.
   A false seam or fixed assumption is NEEDS_REPLAN.
   Tracer code is production-intended; prototypes happen upstream.
   ```

2. **Create an experimental `task/v3-tracer` template**, retaining v3 unchanged as the control.

3. **Add `execution_shape` and the `Tracer bullet` section** to its README.

4. **Rewrite the checklist skeleton** around baseline RED, primary GREEN, same-capability hardening, and integrated gate.

5. **Add `PRIMARY_CMD`** and run it first in `verify.sh`.

6. **Extend `task-lint.sh` and `selftest.sh`** with tracer-specific valid and invalid packages.

7. **Rewrite `TASK-101`** around one seeded-store-to-render-to-quit tracer; move insert and duplicate handling into `TASK-102`.

8. **Split `TASK-106`** into disconnect, terminal restoration, and gallery tasks.

9. **Update the Codex research and goal adapter** to reflect native headed `/goal`.

10. **Add a Grok adapter** that treats the native checklist as non-authoritative.

11. **Run the execution-order and task-boundary ablations** before promoting the schema to v4.

---

# Final recommendation

The best integration is not:

```markdown
## Tracer Bullets

Please work in small vertical slices.
```

That remains an advisory instruction which an agent can ignore while confidently constructing layers.

The stronger design is:

```text
one task = one primary tracer
AC-001 = pre-agreed public seam
baseline = AC-001 RED
first implementation milestone = AC-001 GREEN
later work = hardening of the same capability
new capability = new task and fresh context
false assumption = NEEDS_REPLAN
completion = trusted host verifier
```

That combination preserves everything already strong in `task-format` while shortening the distance between an agent's action and deterministic feedback. Tracer Bullets will not make model behavior deterministic, but they substantially reduce how much unverified work the model can accumulate before reality corrects it—which is the most practical route toward predictable `/goal` execution.

[1]: https://www.aihero.dev/tracer-bullets "https://www.aihero.dev/tracer-bullets"
[2]: https://pragprog.com/tips/ "https://pragprog.com/tips/"
[3]: https://www.aihero.dev/skills-implement "https://www.aihero.dev/skills-implement"
[4]: https://developers.openai.com/cookbook/examples/codex/using_goals_in_codex "https://developers.openai.com/cookbook/examples/codex/using_goals_in_codex"
[5]: https://docs.anthropic.com/en/docs/claude-code/goal "https://docs.anthropic.com/en/docs/claude-code/goal"
[6]: https://x.ai/news/introducing-goal "https://x.ai/news/introducing-goal"
