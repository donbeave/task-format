# Task author gate

**Purpose:** decide whether one proposed ticket is safe to compile into a
`task-format` execution package. This runs before `task-lint.sh`.

- **Initiative:** `<INIT-ID>`
- **Task:** `<TASK-ID>`
- **Reviewer:** `<planner/operator/fresh reviewer>`
- **Status:** `PENDING` <!-- PENDING | APPROVED | REJECTED -->

## 1. Decision integrity

- [ ] The work is already decided; no material product, architecture,
      compatibility, migration, security, or testing decision remains.
- [ ] Every fixed decision has provenance in conversation, a spec, ADR, or
      explicit operator ruling.
- [ ] No assertion was invented to make the task template look complete.
- [ ] Relevant ADRs and project glossary/domain vocabulary were checked.
- [ ] The tracker/repository was searched for overlapping or duplicate work.
- [ ] Binding sources and non-normative orientation hints are separated.
- [ ] Conflicting sources cause rejection/replanning rather than a guessed rule.

## 2. Task boundary

- [ ] The task has exactly one primary independently valuable outcome.
- [ ] It answers: “What can be demonstrated or independently verified when
      only this task is complete?”
- [ ] Its demo answer is behavior or a concrete structural outcome, not
      “the database layer exists,” “tests were added,” or another layer.
- [ ] The task owns every layer required by every AC.
- [ ] No AC can only pass after another unfinished task.
- [ ] Another task could be rejected while this task is approved and shipped.
- [ ] The task fits one fresh small/medium context and normally 5–20 checklist
      leaves.
- [ ] Merge/split alternatives were considered; the chosen granularity was
      explicitly approved.

## 3. Dependencies and frontier

- [ ] Every blocker is a genuine prerequisite, not merely earlier in a list.
- [ ] No hidden blocker remains in prose or assumed conversation context.
- [ ] The graph is acyclic.
- [ ] The task is compiled/dispatched only when all blockers are complete.
- [ ] A prefactor is independently verifiable and names the later tasks it
      actually unblocks.
- [ ] No bookkeeping-only setup ticket exists.

## 4. Verification seams

- [ ] A primary verification seam was selected before task prose was finalized.
- [ ] It is an existing seam where possible.
- [ ] It is the highest stable seam that can prove the outcome.
- [ ] Supporting seams are few and each has a reason the primary seam cannot
      observe the invariant.
- [ ] Prior art in the repository is named.
- [ ] The executor is not allowed to replace the agreed AC seam with a
      lower-level implementation-coupled test.

## 5. Acceptance falsifiability

- [ ] Every AC is classified as `delta`, `invariant`, or `removal`.
- [ ] Every AC names an observation that would prove it false.
- [ ] Every `delta` AC is false/old at the exact base commit.
- [ ] Every `removal` AC is present/active at the exact base commit.
- [ ] Every `invariant` AC passes at the exact base commit.
- [ ] Every AC has one task-owned evidence command and final expectation.
- [ ] The overall verifier fails on the baseline and passes on a reference
      solution.

## 6. Transition policy

- [ ] Direct final migration is used unless an atomic migration cannot keep the
      repository in a verifiable state.
- [ ] Any expand-contract exception is represented in `task-graph.yaml`.
- [ ] The temporary coexistence boundary is exact and bounded.
- [ ] Every migrate batch passes its own task gate.
- [ ] A mandatory contract/removal task exists before the expand is dispatched.
- [ ] Initiative completion cannot occur before the contract task.
- [ ] No indefinite compatibility, fallback, alias, or deprecation promise was
      introduced.

## 7. Compiled package readiness

- [ ] The compiled README contains demo path, independent value, primary seam,
      AC baseline/final matrix, and decision provenance.
- [ ] Exact code paths are resolved at compilation time and scope globs match
      `verify.config`.
- [ ] Read-only decision snapshots and trusted tests are protected.
- [ ] `task-lint.sh` passes.
- [ ] Generated progress matches the immutable checklist.
- [ ] Protected manifest is generated.
- [ ] Baseline/reference gate proof exists.
- [ ] The launch prompt references exactly one task.

## Decision

- **Status:** `PENDING`
- **Blocking findings:** `NONE`
- **Required split/merge/replan:** `NONE`
