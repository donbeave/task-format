# Research message 03 — AIHero `/to-spec` + `/to-tickets` vs `task-format`

Date: 2026-08-28. Snapshot analysed by the research: `9b9aa0c` (pre `d91e6c0`,
i.e. before whitelist-only scope / dropped `protected_paths`). Read
`notes/aihero-spec-tickets.md` for the delta against HEAD; this file is the raw
input, stored verbatim.

Attachments (generated proposal artifacts) live in `03-aihero-attachments/`:

- `TASK-FORMAT-AIHERO-COMPARISON-2026-08-28.md` — full comparison and recommendations
- `TASK-FORMAT-README-v3.1-EXPERIMENTAL.md` — experimental `README.md` task template v3.1
- `TASK-GRAPH-TEMPLATE-v1.yaml` — planner-owned task graph template
- `TASK-AUTHOR-GATE-v1.md` — pre-compilation task author gate
- `TASK-FORMAT-AIHERO-EXPERIMENT-PLAN.md` — ablation and decomposition experiment plan

## Prompt

Now on top of this research, analyze these skills to compare the current shape of:

https://github.com/donbeave/task-format

With these skills:
https://www.aihero.dev/skills-to-spec
https://www.aihero.dev/skills-to-tickets

What we can borrow from these skills to improve the current task template.

## Response

# Executive conclusion

At the current `main` snapshot, `donbeave/task-format` and the two AIHero skills solve **different layers of the same workflow**:

| Component     | Primary responsibility                                                                                 |
| ------------- | ------------------------------------------------------------------------------------------------------ |
| `/to-spec`    | Preserve settled intent and decisions before context is lost                                           |
| `/to-tickets` | Decompose that intent into independently executable vertical slices with blocking edges                |
| `task-format` | Execute one selected task predictably in a fresh context and prove completion through a protected gate |

The best result is therefore **not** to replace the current task template with either AIHero template. Instead, use those skills as a stronger **compiler front end** for `task-format`:

```text
conversation + research + ADRs
              │
              ▼
      decision snapshot / spec
              │
              ▼
 approved tracer-bullet task graph
              │
        choose one READY task
              ▼
 compile one task-format package
              │
              ▼
 fresh Claude Code / Codex context
              │
              ▼
 progress + protected verifier + outer gate
```

This fits the repository's own architecture: the current task package is intentionally isolated from authoring and dispatch tooling, and its research document explicitly leaves the broader task DAG/orchestrator outside the present single-task package.

---

# 1. What `task-format` already does better

The current task package at commit `9b9aa0c65bced7f217faf475a38f5eeff3782603` has a rigorous separation between agent-visible task files and harness-owned authoring/gating tools. The task contract, execution protocol, and verifier are read-only; derived progress lives separately; and the product repository is the only writable implementation workspace.

Its current `README.md` already contains:

* One observable goal.
* Current and desired behavior.
* Exact baseline command and expected pre-change result.
* Executable preconditions.
* Explicit scope and non-goals.
* Normative `R-*` requirements.
* Given/When/Then `AC-*` criteria with evidence commands.
* Fixed `D-*` decisions.
* Numbered hierarchical checkboxes.
* A final protected verifier leaf.

The linter mechanically rejects malformed tasks, placeholders, preconditions without commands, ACs without evidence, invalid checklist hierarchy, fewer than five or more than twenty leaves, ACs not referenced by the checklist, and tasks whose final leaf is not the verifier. It also verifies that scope globs match the verifier configuration.

The generic verifier is substantially stronger than an ordinary issue checklist. It validates protected-file hashes, changed-file scope, required and forbidden paths, forbidden patterns, focused tests, regressions, linting, and progress consistency. The host reruns the same verifier against trusted task copies, and only exit code `0` with final line `DONE` is authoritative.

The current pgtui experiments are also already close to tracer-bullet decomposition. For example, TASK-101 crosses persistence, application state, key handling, rendering, runtime, and CLI lifecycle to deliver one independently visible "list saved connections" outcome rather than splitting those layers into unrelated tickets.

So the main improvement area is **not another completion protocol**. It is ensuring that the planner selected the correct unit of work before producing the task package.

---

# 2. What `/to-spec` contributes

## 2.1 Treat the specification as a decision snapshot

`/to-spec` does not conduct a new interview. It captures decisions already made in the conversation, repository, project context, and ADRs. Its documentation explicitly treats newly invented assertions as defects. The spec exists because the original context window will eventually disappear. ([AI Hero][1])

This should become a hard task-authoring rule:

> A task compiler may import decisions, but it must not silently create missing product, architecture, compatibility, migration, security, or testing decisions while filling the task template.

The current task template has `D-*` decisions, but their **provenance is not standardized**. That creates a subtle risk: a task generator can fill an empty decision section with a plausible implementation choice and make it appear operator-approved.

### Recommended change

Use a provenance column:

```markdown
## Fixed decisions

| ID | Decision | Provenance |
| --- | --- | --- |
| D-001 | Use the existing `TokenStore::validate`. | `ADR-041 §3` |
| D-002 | Error code is exactly `refresh_token_expired`. | spec `S-17`, AC-4 |
| D-003 | Delete the legacy path; no feature flag. | operator ruling `DEC-092` |
```

For larger tasks, formalize the pattern already used by the pgtui experiments:

```text
/task/decisions.md
```

That file should be:

* Planner-owned.
* Read-only.
* Protected by the manifest.
* A local snapshot of the relevant decisions.
* Structured with stable global `D-*` IDs.
* Explicit about source/provenance.

The pgtui tasks already ship a separate `decisions.md`, but the current reference template does not yet define its contract or require decision provenance.

## 2.2 Separate binding sources from orientation hints

The current template labels the complete "Read before editing" list as non-normative. Yet the example task places a decision record in that list and says its contract is already decided. The pgtui example similarly calls `/task/decisions.md` a non-normative hint while simultaneously requiring its decisions verbatim.

That should be corrected to:

```markdown
Binding sources — conflict means NEEDS_REPLAN:

1. `/task/decisions.md` — imported D-* decisions.
2. `/task/contracts/auth-refresh.md` — public error contract.

Orientation hints — informative:

1. `src/auth/session/rotate.rs` — current flow.
2. `tests/auth/refresh_test.rs` — testing prior art.
```

This makes authority resolution unambiguous after context compaction.

## 2.3 Select verification seams before writing implementation tasks

The most valuable `/to-spec` concept is **seams before prose**. It chooses the testing boundary before drafting the specification, prefers existing seams, and uses the highest stable seam that can prove the behavior. Its downstream implementation skill then performs TDD at those pre-agreed seams, and review checks the implementation against the specification. ([AI Hero][1])

The current task format has exact evidence commands, but does not explain **why those commands are the correct proof surface**. An executor can therefore add a convenient low-level unit test that passes while the user-facing boundary remains broken.

### Recommended addition under `Context`

```markdown
**Primary verification seam:** `POST /auth/refresh` process/API boundary  
**Why this seam:** It proves the user-visible status, response body, and
failure behavior without depending on private helper structure.  
**Supporting seams:** Database-state inspection is additionally required
because the HTTP response cannot prove that no session mutation occurred.  
**Prior art:** `tests/auth/valid_refresh_rotation.rs`
```

The ideal is one primary seam, but this should not become dogma. Some outcomes require a direct-use seam plus a second state-inspection seam to prove an otherwise invisible invariant.

The executor may add supporting unit tests, but it should not be allowed to replace the agreed acceptance seam with a lower implementation-coupled seam.

## 2.4 Preserve real refusals

The current `Out of scope` section should remain. `/to-spec` correctly emphasizes that decisions deliberately refused are often among the most useful parts of a specification. ([AI Hero][1])

The author gate should reject generic-only non-goals such as:

```text
Unrelated cleanup is out of scope.
```

and require at least the real adjacent work that was considered and excluded:

```text
Access-token expiry is excluded.
Session-storage redesign is excluded.
A backward-compatible dual validator is explicitly rejected.
```

## 2.5 Do not copy the long user-story catalogue

The source skill asks for a very long numbered user-story list. Its own documentation acknowledges that this structure fits refactors and module-boundary changes poorly.  ([AI Hero][1])

For one fresh `/goal` execution, the existing one-sentence Goal plus behavioral ACs is better. At most, add one short actor/value statement. A long story catalogue would:

* Increase task-token weight.
* Duplicate acceptance criteria.
* Encourage multiple independently completable outcomes.
* Be especially artificial for refactors and removals.

---

# 3. What `/to-tickets` contributes

## 3.1 Make the task a tracer bullet, not a layer

`/to-tickets` defines each ticket as a narrow but complete path through every layer needed to make one behavior work. Each ticket should be independently demonstrable, independently verifiable, and small enough for one fresh context. It explicitly rejects splitting work into "database first, API second, UI third, tests fourth." ([AI Hero][2])

The current template says "one coherent behavior or vertical slice," but this is not mechanically or procedurally enforced.

### Recommended direct addition

Under `Goal`:

```markdown
**Demo path:** `pgtui --db <fixture>` → the saved connections screen appears
and `q` exits successfully.

**Independent value:** After this task alone, operators can inspect saved
connections without creating or connecting to a new one.
```

The required authoring question becomes:

> What can I demonstrate when only this task is complete?

A task with no answer is probably:

* A horizontal layer.
* Bookkeeping.
* Over-decomposed.
* Dependent on unfinished work.
* Not independently valuable.

## 3.2 Add blocking edges—but outside the executor README

`/to-tickets` makes dependency edges central. A ticket is ready when all of its blockers are complete, and any task on that dependency frontier may be dispatched. ([AI Hero][2])

This is valuable, but the current `task-format` research intentionally removed `depends_on` and broader DAG metadata from the agent-visible frontmatter because no in-container component consumed them. Adding decorative fields would consume attention without enforcing anything.

The right addition is a planner/harness-owned sidecar:

```text
plans/<initiative>/task-graph.yaml
```

Example:

```yaml
schema: task-graph/v1

tasks:
  - id: TASK-101
    shape: tracer-bullet
    delivers: "List persisted connections in the TUI"
    demo_path: "launch with a seeded store and observe the list"
    primary_seam: "TUI process/render boundary"
    blocked_by: []
    owns_source_ids: [REQ-001, REQ-002]
    package: tasks/TASK-101
    author_gate:
      status: approved

  - id: TASK-102
    shape: tracer-bullet
    delivers: "Create and persist a connection"
    blocked_by: [TASK-101]
    package: tasks/TASK-102
```

This graph should not be mounted wholesale into `/task`. The harness selects one frontier task and compiles only task-relevant dependency facts into its baseline or preconditions.

That also generalizes the current pgtui experiment arrangement, where dependencies are effectively encoded by derived fixtures—`pgtui-102` contains the TASK-101 reference result, and so on—but not represented as a reusable machine-readable graph.

## 3.3 Add an explicit decomposition approval gate

Before `/to-tickets` publishes anything, it presents the proposed split and asks the operator to review granularity, blockers, and possible merges or splits. It also searches for useful prefactoring before the main work. ([AI Hero][2])

`task-format` needs an equivalent authoring stage **before** `task-lint.sh`.

For each task, the reviewer should see:

| Field            | Question                                                  |
| ---------------- | --------------------------------------------------------- |
| Title            | Is this one result rather than a work category?           |
| What it delivers | What becomes usable?                                      |
| Demo path        | How can this result be shown independently?               |
| Primary seam     | Where is the result proven?                               |
| Blocked by       | Are these genuine prerequisites?                          |
| Context fit      | Can one fresh agent finish it?                            |
| Merge/split      | Would a neighboring task be better combined or separated? |

The operator approves the graph first. Only then are task packages compiled and linted.

The current linter is excellent at validating an already-compiled contract, but it cannot infer whether that contract is the correct slice of the parent initiative.

## 3.4 Add per-criterion baseline falsifiability

This is the **highest-value direct template change**.

The current harness proves that the overall verifier fails on the untouched fixture and passes on a reference solution. That is already strong. But it does not prove that each individual AC is meaningful. `/to-tickets` documentation identifies the recurring failure where an AC already passes before implementation, belongs to another ticket, or merely restates the request. It recommends naming the observation that would make every criterion false and checking that observation at the starting commit. ([AI Hero][2])

Change the current AC table from:

```markdown
| ID | Given / When / Then | Evidence command | Expected |
```

to:

```markdown
| ID | Class | Given / When / Then | Evidence command | Baseline expected | Final expected |
| --- | --- | --- | --- | --- | --- |
| AC-001 | delta | ... | `...` | old behavior / FAIL | new behavior / PASS |
| AC-002 | invariant | ... | `...` | PASS | PASS |
| AC-003 | removal | ... | `...` | legacy path present | legacy path absent |
```

The classes mean:

* **`delta`**: requested behavior is false or old at baseline and correct finally.
* **`invariant`**: supported behavior passes both before and after.
* **`removal`**: old behavior or artifact is present before and absent finally.

This avoids the incorrect rule that every AC must fail at baseline. Regression invariants should pass before and after. What matters is that every criterion has an explicit polarity.

The checklist baseline leaf should then require these observations to be recorded before implementation:

```markdown
- [ ] **1.2** Baseline command and AC baseline observations are recorded —
  evidence: each delta/removal shows its declared pre-change state and each
  invariant shows its declared passing result.
```

## 3.5 Prefactor only when it produces a real outcome

The "make the change easy before making the easy change" idea is useful, but it can easily create meaningless setup tickets.

A prefactor should become a separate task only when:

* It produces an independently reviewable structural result.
* Existing behavior remains green.
* It creates an explicit seam or boundary later tasks consume.
* Its graph edges identify exactly which tasks it unblocks.
* It is not merely scaffolding that should be folded into the tracer bullet.

## 3.6 Use the wide-refactor exception narrowly

`/to-tickets` has an important exception for very wide mechanical changes:

```text
expand → migrate batches → contract
```

This conflicts with the current task requirement to implement the final design directly without dual paths or compatibility layers. ([AI Hero][2])

The correct synthesis is:

### Default

Keep the existing direct-migration rule.

### Exception

Allow expand–migrate–contract only when an atomic direct migration cannot keep the repository in a verifiable state.

The graph must then establish:

1. One explicit expand task.
2. One or more bounded migrate tasks.
3. One mandatory contract/removal task.
4. Every migrate task blocked by expand.
5. Contract blocked by all migrations.
6. Initiative completion blocked by contract.
7. Exact temporary coexistence surface.
8. No indefinite compatibility or fallback promise.
9. Every individual task still passes its own protected verifier.

When migration batches cannot remain green independently and require an integration branch, that is outside the current single-task harness. The solution is an initiative-level integration harness—not weakening individual `verify.sh` gates.

---

# 4. What should change in the task template

## Direct task-template changes

| Priority | Change                                      | Reason                                                              |
| -------- | ------------------------------------------- | ------------------------------------------------------------------- |
| P0       | `Demo path`                                 | Makes verticality and independent usefulness visible                |
| P0       | `Independent value`                         | Prevents layer-only or bookkeeping tasks                            |
| P0       | Primary and supporting verification seams   | Binds implementation and review to an agreed proof surface          |
| P0       | AC class plus baseline/final expectation    | Detects criteria that grade nothing                                 |
| P0       | Decision provenance                         | Prevents invented planner decisions                                 |
| P0       | Binding sources separated from hints        | Removes authority ambiguity                                         |
| P1       | Explicit task shape                         | Distinguishes tracer bullet, prefactor, and bounded migration roles |
| P1       | Bounded transition wording                  | Allows rare wide refactors without normalizing compatibility layers |
| P2       | Durable-learning promotion after completion | Moves lasting discoveries into ADRs or project context              |

The proposed experimental README is approximately 7.2 KB, below the current linter's 10 KB warning threshold, so these changes do not require returning to the oversized v2-style contract.

## Harness and authoring changes

| Artifact                 | Ownership                 | Purpose                                                               |
| ------------------------ | ------------------------- | --------------------------------------------------------------------- |
| `task-graph.yaml`        | Planner/harness           | Blocking edges, frontier, slice shape, requirement ownership          |
| `task-author-gate.md`    | Planner/operator/reviewer | Validate decision fidelity, verticality, seams, blockers, AC polarity |
| `decisions.md`           | Planner, read-only        | Local decision snapshot with provenance                               |
| `task-lint.sh` additions | Harness                   | Enforce structured demo/seam/AC/provenance fields                     |
| `graph-lint.sh`          | Harness                   | Validate DAG, blockers, transition closure, package references        |

---

# 5. What should not be borrowed

## Do not place the complete spec in the executor context

`/to-spec` is intended for work spanning multiple sessions. The full spec is the durable parent reasoning artifact; the fresh executor needs only the relevant compiled projection. AIHero itself distinguishes durable specs from disposable one-context tickets. ([AI Hero][1])

## Do not remove exact paths from the compiled task

Both AIHero skills avoid specific file paths because durable tracker tickets can outlive repository structure. That is appropriate upstream.

But exact paths and globs are load-bearing in `task-format`:

* They orient the fresh executor.
* They define expected scope.
* They feed `verify.config`.
* They let the host reject unrelated changes.

Use a two-stage policy:

```text
durable spec/ticket: path-light
compiled task package: exact paths resolved and snapshotted at dispatch
```

## Do not make the parent spec dispatchable

AIHero documents a known rough edge where a parent spec marked `ready-for-agent` may be picked up by an AFK agent and implemented as one giant task. Only leaf task packages should enter the execution frontier. ([AI Hero][1])

Recommended lifecycle:

```text
SPEC: approved-for-decomposition
TICKET: ready-for-compilation
TASK PACKAGE: ready-for-agent
```

## Do not rely exclusively on tracker text

AIHero also notes that large tracker-hosted specifications can truncate and that specs become stale as implementation teaches new facts. A local protected decision snapshot is safer for execution. Durable discoveries should be promoted separately to ADRs or project context, not retroactively hidden inside a mutable task. ([AI Hero][1])

## Do not replace executable evidence with checkboxes

The current read-only contract, generated progress file, trusted verifier, baseline/reference oracle, and host rerun are stronger than ordinary ticket acceptance checkboxes and should remain the completion authority.

## Do not put the decomposition quiz inside `/goal`

By the time `/goal` starts:

* Granularity is approved.
* Blockers are resolved.
* The seam is selected.
* Decisions are fixed.
* The verifier already exists.
* The task package is immutable.

An executor discovering that the slice is wrong should return `NEEDS_REPLAN`, not redesign the graph.

---

# 6. Recommended final flow

```text
1. Decide
   conversation + research + ADRs

2. Snapshot
   decision-faithful spec; no invented assertions

3. Decompose
   tracer-bullet task graph; blockers; prefactors; bounded exceptions

4. Review
   operator approves granularity, edges, demos, and seams

5. Compile
   one READY graph node into task-format README + decisions + verifier config

6. Author gate
   provenance, verticality, AC ownership, baseline polarity, transition closure

7. Prove the oracle
   verifier FAIL on baseline; PASS with reference solution

8. Dispatch
   one fresh context with /task mounted read-only

9. Execute and gate
   generated progress + protected verifier + trusted host rerun

10. Review and learn
   fresh code review where required; promote lasting facts to ADR/CONTEXT
```

---

# 7. Experimental adoption order

Because `task-format` is an empirical research project, these changes should not all be merged simultaneously.

Recommended ablations:

| Variant | Change                                     |
| ------- | ------------------------------------------ |
| A0      | Current `task/v3`                          |
| A1      | A0 + demo path and independent value       |
| A2      | A0 + primary/supporting verification seams |
| A3      | A0 + AC baseline/final polarity            |
| A4      | A0 + decision provenance                   |
| A5      | All direct P0 additions                    |

Measure:

* Host-gate pass rate.
* First-pass verifier success.
* Verifier reruns.
* Turns, cost, and tool calls.
* Out-of-scope changes.
* Protected-input tamper attempts.
* Low-level test substitution.
* Checked leaves without evidence.
* Demo-path execution.
* Independent-review findings.

A separate decomposition experiment should compare the current authoring process against tracer-bullet instructions, required demo paths, blocking edges, and the complete author gate. Blind reviewers should score verticality, dependency correctness, AC ownership, task count, context fit, and transition closure.

The strongest immediate candidate is **AC baseline/final polarity** because it addresses a concrete false-acceptance class while adding little context. The strongest upstream candidate is the **author-approved task graph with demo paths, seams, and blocking edges**.

---

# Generated proposal artifacts

Stored under `03-aihero-attachments/`:

- `TASK-FORMAT-AIHERO-COMPARISON-2026-08-28.md` — full comparison and recommendations
- `TASK-FORMAT-README-v3.1-EXPERIMENTAL.md` — experimental `README.md` task template v3.1
- `TASK-GRAPH-TEMPLATE-v1.yaml` — planner-owned task graph template
- `TASK-AUTHOR-GATE-v1.md` — pre-compilation task author gate
- `TASK-FORMAT-AIHERO-EXPERIMENT-PLAN.md` — ablation and decomposition experiment plan

The recommended design direction is:

> **Keep `task-format` as the strict execution backend; borrow decision fidelity and seam selection from `/to-spec`, and borrow tracer-bullet decomposition, blocker graphs, demo paths, and per-criterion falsifiability from `/to-tickets`.**

[1]: https://www.aihero.dev/skills-to-spec "https://www.aihero.dev/skills-to-spec"
[2]: https://www.aihero.dev/skills-to-tickets "https://www.aihero.dev/skills-to-tickets"
