# `task-format` compared with AIHero `/to-spec` and `/to-tickets`

**Analysis date:** 2026-08-28  
**`task-format` snapshot:** `donbeave/task-format@9b9aa0c65bced7f217faf475a38f5eeff3782603`  
**Skill snapshots:**  
- `mattpocock/skills` `to-spec/SKILL.md` blob `3f52599ae2a4347aee5a07432c2707518e691a7f`
- `mattpocock/skills` `to-tickets/SKILL.md` blob `e868c831fcfb1e124e010bcdf84a429ec879160f`

## Executive verdict

These are not competing task formats.

- **`/to-spec` is a decision-snapshot authoring skill.**
- **`/to-tickets` is a decomposition and dependency-graph skill.**
- **`task-format` is an execution contract and verification harness for one
  already-selected task.**

The strongest combined architecture is:

```text
conversation + ADRs + repository exploration
                    │
                    ▼
        immutable decision snapshot / spec
                    │
                    ▼
 approved tracer-bullet task graph with blocking edges
                    │
             choose one READY leaf
                    ▼
       compile one read-only task-format package
                    │
                    ▼
       fresh Claude Code / Codex execution context
                    │
                    ▼
    progress ledger + protected verifier + outer gate
```

Do not paste the full `/to-spec` output or the full ticket graph into the
fresh executor's `README.md`. That recreates the broad-context failure
`task-format` exists to avoid.

The direct template changes should be small:

1. Add an explicit **demo path** and **independent value**.
2. Add one agreed **primary verification seam**, with justified supporting
   seams.
3. Give every acceptance criterion a **baseline polarity** and a final
   expectation.
4. Give every fixed decision **provenance**.
5. Make the task's execution shape explicit.
6. Keep blockers and initiative decomposition in a planner/harness-owned
   `task-graph.yaml`, not as dead fields in the agent-visible task.
7. Add an author gate that rejects horizontal slices, invented decisions,
   criteria owned by future tasks, and transition plans with no mandatory
   contract/removal task.

---

# 1. Current `task-format` shape

The current repository has a disciplined separation:

```text
reference/task-template/   only files visible to the executor
harness/                   authoring, dispatch, lint, progress, manifest, gate
experiments/               fixtures, task packages, results
docs/research/             evidence and decisions
```

One executor receives:

```text
/task/README.md       read-only task contract
/task/AGENTS.md       read-only shared execution protocol
/task/verify.sh       read-only canonical gate
/task/verify.config   read-only task-specific gate inputs
/task/protected.sha256
/progress/progress.md read-write derived state
/work/                read-write product repository
```

The current task contract already contains:

- one observable goal;
- current and desired behavior;
- exact baseline command and expected failure;
- executable preconditions;
- in-scope and out-of-scope boundaries;
- normative `R-*` requirements;
- Given/When/Then `AC-*` rows with evidence commands;
- fixed `D-*` decisions;
- hierarchical numbered checkboxes;
- a protected final verifier.

The linter already enforces:

- required sections and no placeholders;
- executable preconditions;
- evidence and expected results for every AC;
- 5–20 evidence-bearing leaves;
- contiguous IDs and maximum depth four;
- AC-to-checklist coverage;
- a final verifier leaf;
- frontmatter/verify-config path consistency;
- a task-size warning above approximately 2,500 tokens.

The harness additionally requires the verifier to fail on the baseline and
pass on a reference solution.

This is much stronger than the execution contract provided by the AIHero
skills. The gap is upstream: `task-format` currently says its DAG/orchestrator
is out of scope, and task readiness does not mechanically include tracer-bullet
quality, test-seam agreement, task-graph edges, or human approval of the split.

---

# 2. What `/to-spec` contributes

## 2.1 A spec is a decision snapshot, not a decision-making session

`/to-spec` deliberately does not interview. It synthesizes decisions that were
already made from conversation, repository context, project vocabulary, and
ADRs. An assertion that was never agreed is considered a defect.

### Borrow

Add a planner-side rule:

> A compiled task may import decisions, but the task author must not create
> missing product, architecture, compatibility, migration, security, or
> testing decisions while filling the template.

Every `D-*` should name its provenance:

```markdown
| ID | Decision | Provenance |
| --- | --- | --- |
| D-001 | Use the existing `TokenStore::validate`. | `ADR-041 §3` |
| D-002 | Error code is `refresh_token_expired`. | operator decision, spec `S-17` |
```

For large decision sets, formalize the pattern already used by the pgtui
experiments:

```text
/task/decisions.md    read-only, protected, planner-owned snapshot
```

The task README imports only the relevant D IDs. `decisions.md` must carry
source references and must be protected by the dispatch manifest.

## 2.2 Use project vocabulary and respect ADRs

This should be an authoring rule, not executor prose. Add to the author gate:

- repository glossary/domain nouns were used;
- relevant ADRs were searched;
- overlapping tracker work was searched;
- imported decisions cite their source;
- conflicts cause re-planning rather than silent synthesis.

This improves on `/to-spec`, whose documentation acknowledges that it respects
ADRs but does not cite them and does not search the tracker for overlap.

## 2.3 Agree on test seams before writing the task

`/to-spec` sketches test seams before prose, preferring existing seams and the
highest stable seam possible.

The current task format has exact evidence commands, but not the planner's
reason for choosing their proof surface. Add a compact task field:

```markdown
Primary verification seam: `<public boundary or highest stable seam>`
Why this seam: <why it proves the outcome without coupling to internals>
Supporting seams: `none` or <seams required for otherwise invisible invariants>
Prior art: `<existing test/helper/path>`
```

Rules:

- Acceptance proof uses the primary seam whenever feasible.
- Supporting unit/component seams are allowed only when the primary seam cannot
  observe an important invariant.
- The executor may add task-local tests, but may not replace the agreed
  acceptance seam with a lower implementation-coupled seam.
- “One seam” is a preference, not an absolute rule. A database mutation plus a
  UI result may require a direct-use seam and a state-inspection seam.

## 2.4 Preserve refusals and non-goals

`task-format` already has Out of scope. Keep it. Strengthen the author gate so
out-of-scope entries include real rejected adjacent work, not generic
“unrelated cleanup” boilerplate.

## 2.5 Do not import the long user-story list

The `/to-spec` source template requests an extremely long numbered user-story
list. Its documentation itself notes that this shape works poorly for
refactors and module-boundary work.

For one `/goal` execution, `task-format`'s one-sentence goal plus observable
acceptance criteria is better. At most add one actor/value statement under
“Independent value”; do not add a long story catalogue to the execution
contract.

---

# 3. What `/to-tickets` contributes

## 3.1 Tracer-bullet vertical slices

A ticket should cut a narrow but complete path through every layer required to
make one behavior work. It must answer:

> What can be demonstrated or independently verified when only this ticket is
> complete?

The current template mentions a “coherent behavior or vertical slice,” and the
pgtui experimental tasks are generally vertical, but the linter does not
validate this property.

### Borrow

Add two required lines to the task:

```markdown
Demo path: `<command or interaction>` → `<observable result>`
Independent value: <what is useful after this task alone>
```

Add author-gate checks:

- all layers needed by the demo are owned by this task;
- no AC can pass only after another pending task;
- no task is merely “database,” “API,” “UI,” or “tests” unless it is explicitly
  a prefactor or wide-refactor step;
- the task remains useful and green when landed alone.

## 3.2 Blocking edges and the frontier

Blocking edges are highly valuable, but they should not be decorative text in
the executor README.

The current research intentionally removed DAG fields from task frontmatter
because no in-container component consumed them. Preserve that decision until
the harness gains a graph consumer.

Add a planner/harness-owned artifact instead:

```text
plans/<initiative>/task-graph.yaml
```

It records:

- task ID and title;
- execution shape;
- what it delivers;
- demo path;
- primary seam;
- `blocked_by`;
- source requirement ownership;
- path to the compiled task package;
- transition role when applicable;
- author approval.

The graph must not be mounted into `/task` unless the executor needs a specific
fact from it. At dispatch, the harness selects only frontier tasks—tasks whose
blockers are all complete.

This keeps the fresh context small while making dependencies machine-readable.

## 3.3 Human review of granularity and edges

Before publishing or compiling task packages, show the proposed list with:

- Title
- Blocked by
- What it delivers
- Demo path
- Primary seam
- Why it fits one fresh context

The operator approves:

- task size;
- blockers;
- merge/split decisions;
- prefactor ordering;
- any wide-refactor exception.

This approval belongs before `task-lint.sh`. The task linter validates a
compiled package; it cannot determine whether the planner chose the right
slice from a larger initiative.

## 3.4 Prefactor before feature work

Borrow the rule, but constrain it:

- A prefactor becomes its own task only when it produces an independently
  verifiable structural outcome that clearly unblocks later slices.
- Do not create bookkeeping-only “setup” tickets.
- The prefactor must preserve current behavior and have a concrete proof.
- Its blocker edges must show which tracer-bullet tasks it actually unlocks.

## 3.5 Per-criterion baseline falsifiability

This is the most valuable direct template improvement.

The current harness proves the **overall verifier** fails on the untouched
baseline and passes on a reference solution. That does not guarantee every AC
is meaningful. Some invariant criteria may already pass; other criteria can be
so weak they pass before implementation.

Change the AC table to:

```markdown
| ID | Class | Given / When / Then | Evidence command | Baseline expected | Final expected |
| --- | --- | --- | --- | --- | --- |
| AC-001 | delta | ... | `...` | FAIL: old behavior | PASS: new behavior |
| AC-002 | invariant | ... | `...` | PASS | PASS |
| AC-003 | removal | ... | `...` | symbol present | symbol absent |
```

Classes:

- `delta` — requested behavior is false at baseline and true finally;
- `invariant` — supported behavior passes both before and after;
- `removal` — old behavior/artifact is present before and absent finally.

Author-gate rule:

> For every criterion, name the observation that would prove it false and
> record the expected observation at the exact base commit.

Checklist baseline work should record these observations in `progress.md`.
The final verifier still remains the completion oracle.

## 3.6 Wide-refactor exception

Do not copy expand–contract as a general preference. It conflicts with the
project's deliberate “final design directly; no dual path” rule.

Adopt only this bounded exception:

1. Use it only when an atomic direct migration cannot keep the repository in a
   verifiable state.
2. Represent the sequence in `task-graph.yaml`:
   `expand → migrate batches → contract`.
3. Every task must still pass its own canonical gate.
4. The temporary old/new coexistence is explicitly named and bounded.
5. A mandatory contract/removal task exists before dispatching the expand.
6. Initiative completion is impossible while the contract task is incomplete.
7. No indefinite compatibility promise, deprecation period, or fallback is
   introduced.

When migration batches cannot pass independently and require a shared
integration branch, that workflow is outside the current one-task harness. Do
not weaken a task verifier to accommodate it; add an initiative-level
integration harness first.

---

# 4. Comparison matrix

| Concern | Current `task-format` | `/to-spec` | `/to-tickets` | Combined direction |
| --- | --- | --- | --- | --- |
| Decision capture | Fixed D IDs; large experiments use `decisions.md`, but provenance is not standardized | Core strength: capture only agreed decisions | Consumes settled context | Standardize protected decision snapshot and D provenance |
| Domain vocabulary / ADRs | Read hints and decisions, not author-gated | Explicitly required | Explicitly required | Add planner author gate and overlap search |
| One-task execution | Core purpose | Spec may span sessions | One ticket per fresh context | Compile one READY ticket into task-format |
| Vertical slicing | Mentioned; examples often vertical; not linted | Not primary | Core rule | Add demo path and verticality author gate |
| Dependency graph | Explicitly out of current package scope | None | Core rule | Add non-mounted `task-graph.yaml` |
| Test seam selection | Exact evidence commands; no seam rationale | Core rule: agree first, highest existing seam | Ticket must be verifiable | Add primary seam + supporting-seam rationale |
| AC falsifiability | Overall gate must fail baseline; per-AC polarity absent | Not validated | Documentation identifies this failure mode | Add delta/invariant/removal baseline matrix |
| Paths | Exact paths/globs are load-bearing for scope and context | Avoids paths in durable spec | Avoids paths in durable tickets | Keep upstream artifacts path-light; resolve paths in compiled task |
| Progress/recovery | Strong generated ledger | Not covered | Not covered | Keep task-format unchanged |
| Tamper resistance | Read-only package, hashes, outer rerun | Not covered | Not covered | Keep task-format unchanged |
| Verifier quality | Strong generic gate plus baseline/reference oracle | Testing decisions only | AC checkboxes only | Keep task-format as authority |
| Human split approval | Manual authoring but no formal split quiz | Seams checked with user | Granularity/edges explicitly approved | Add author gate before compile/lint |
| Refactors | Direct final design; no compatibility path | User-story template is weak for refactors | Bounded wide-refactor exception | Preserve direct default; allow explicit bounded exception |
| Durable learning | Research docs and task evidence; no promotion field | Durable knowledge should move to CONTEXT/ADRs | Tickets disposable | Add final “durable learnings” promotion step outside task state |

---

# 5. Changes to make directly in the execution task

## P0 — recommended for the next experimental schema

### 5.1 Add delivery shape, demo path, and independent value

```yaml
shape: tracer-bullet
```

Allowed values:

```text
tracer-bullet
prefactor
wide-refactor-expand
wide-refactor-migrate
wide-refactor-contract
integration
```

Under Goal:

```markdown
Demo path: `<command or interaction>` → `<observable result>`

Independent value: <what works after this task alone>
```

### 5.2 Add verification-seam metadata

Under Context:

```markdown
Primary verification seam: `<boundary>`
Why this seam: <rationale>
Supporting seams: `none` or <bounded list + reason>
Prior art: `<existing test/helper>`
```

### 5.3 Add AC class and baseline/final expectations

Replace the four-column AC table with the six-column table described above.

### 5.4 Add decision provenance

Use either a compact table in README or a protected `decisions.md`.

### 5.5 Split binding sources from orientation hints

The current examples label the complete “Read before editing” list
non-normative while also putting binding decision records in it. That is
ambiguous.

Use:

```markdown
Binding sources:
1. `/task/decisions.md` — imported D IDs; conflict means NEEDS_REPLAN.

Orientation hints:
1. `src/...` — current flow.
2. `tests/...` — prior art.
```

## P1 — authoring/harness layer

- Add `task-graph.yaml`.
- Add `author-gate.md`.
- Add `graph-lint.sh`.
- Make `task-lint.sh` require shape/demo/seam/AC polarity/decision provenance.
- Have dispatch refuse a task whose blockers are incomplete or author gate is
  not approved.
- Search tracker and repository for overlapping work before graph approval.
- Record operator approval of granularity and edges.

## P2 — completion and learning

After a task passes:

- identify implementation discoveries that invalidate or refine upstream
  assumptions;
- promote durable facts to `CONTEXT.md` or an ADR through a separate reviewed
  change;
- do not mutate the original spec/task contract retroactively;
- update future, not-yet-dispatched task packages and re-run their author gate.

---

# 6. What not to borrow

## Do not add a long user-story catalogue to `/task/README.md`

It adds context weight, duplicates ACs, and performs poorly for refactors.

## Do not remove exact file paths from the compiled task

Path-light specs and tickets are good durable planning artifacts. The compiled
task package is different: exact code pointers and path globs are inputs to
scope enforcement and fast orientation.

Use a two-stage rule:

```text
durable spec/ticket: path-light
compiled execution package: paths resolved and snapshotted at dispatch
```

## Do not mark the parent spec as executable

Only leaf tickets/task packages should be dispatchable. A parent spec labeled
for generic AFK agents risks one agent attempting the whole initiative.

## Do not use tracker issue text as the only execution source

Tracker bodies may be edited, become stale, or truncate. Compile the relevant
decisions into a local read-only package.

## Do not replace executable evidence with acceptance checkboxes

The existing protected verifier, baseline/reference oracle, progress
consistency check, and outer rerun are stronger and should remain load-bearing.

## Do not ask the `/goal` executor to approve decomposition

Granularity, seams, blockers, and transition shape must be approved before the
fresh execution context starts.

## Do not adopt expand–contract by default

The direct final migration remains the default. Expand–contract is a narrowly
bounded graph-level exception, not a compatibility policy.

---

# 7. Recommended final flow

```text
1. Decide
   conversation + research + ADRs

2. Snapshot
   decision/spec artifact; no invented assertions

3. Decompose
   tracer-bullet graph; blockers; prefactors; explicit exceptions

4. Review decomposition
   operator approves granularity, edges, demo paths, seams

5. Compile
   one READY ticket → README.md + decisions.md + verify.config + trusted tests

6. Author gate
   verticality, ownership, baseline polarity, provenance, package readiness

7. Prove oracle
   verify fails baseline and passes reference solution

8. Dispatch
   one fresh /goal context with read-only /task

9. Gate
   generated progress + protected verifier + host rerun

10. Review and learn
   optional fresh code review; promote durable learning to ADR/CONTEXT
```

---

# 8. Experimental adoption plan

Because `task-format` is an empirical research project, do not merge all
changes and attribute any improvement to the bundle.

## Experiment A — execution-template ablations

Use the same fixtures, seeds, models, turn/budget limits, and verifier.

| Variant | Change |
| --- | --- |
| A0 | Current `task/v3` |
| A1 | A0 + explicit demo path / independent value |
| A2 | A0 + primary verification seam |
| A3 | A0 + AC class and baseline/final expectations |
| A4 | A0 + decision provenance |
| A5 | All P0 additions |

Measure:

- outer-gate pass rate;
- turns, cost, tool calls, and verifier reruns;
- out-of-scope file attempts;
- protected-input tamper attempts;
- implementation-coupled test creation;
- false checkbox completion;
- post-run reviewer findings;
- whether the agent demonstrates the task through the declared demo path.

## Experiment B — decomposition quality

Give the same multi-feature specification to several planner runs.

Compare:

- current unstructured authoring;
- tracer-bullet instructions only;
- tracer-bullet + demo path;
- full author gate + graph lint.

Blind reviewers score:

- independent demoability;
- vertical versus horizontal shape;
- AC ownership;
- blocker correctness;
- context-window fit;
- unnecessary task count;
- overlap/duplicate work;
- transition closure for wide refactors.

Only after these experiments should `task/v3.1` replace `task/v3`.

---

# Final recommendation

Keep the current executor protocol and verifier mostly unchanged.

The AIHero skills reveal that the next reliability frontier is not a longer
execution prompt. It is a better compiler from settled intent to one task:

> **decision-faithful spec → approved tracer-bullet graph → one falsifiable,
> demoable, seam-aware task package.**

The best immediate direct change is the per-AC baseline/final matrix. The best
upstream change is a task graph with demo paths, primary seams, blocking edges,
and an operator-approved author gate.
