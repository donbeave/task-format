# AIHero `/to-spec` + `/to-tickets` vs `task-format` — what is new (delta note)

Research note, 2026-08-28. Input: `docs/research/raw/03-aihero-spec-tickets.md` (verbatim research reply) and its attachments in `raw/03-aihero-attachments/` (comparison, experimental README v3.1, task-graph template, author gate, experiment plan). Skills analysed: https://www.aihero.dev/skills-to-spec and https://www.aihero.dev/skills-to-tickets (source blobs `to-spec/SKILL.md` `3f52599`, `to-tickets/SKILL.md` `e868c83` in `mattpocock/skills`).

The research analysed snapshot `9b9aa0c`. HEAD is `d91e6c0` (whitelist-only scope, D23). This note keeps only what `RESEARCH-FINDINGS.md` and the v3 package do not already contain, and marks research claims that are stale against HEAD. Nothing here is adopted into `task/v3`; every item is a hypothesis for §7/§8 of the findings.

Verdict carried over unchanged: the two skills are upstream of `task-format` (decision snapshot → tracer-bullet decomposition → one compiled task), not competing task formats. The executor package, protocol, generated `progress.md`, verifier, oracle rule (D13) and host rerun stay the completion authority.

---

## 1. Delta matrix

| Research proposal | Status vs HEAD | Where / what is missing |
| --- | --- | --- |
| Executor package is read-only, verifier + host rerun authoritative, progress generated | PRESENT | D1, D12–D14; `harness/README.md` Gate |
| Linter rules cited (sections, placeholders, P-* commands, AC evidence, 5–20 leaves, depth 4, AC coverage, gate leaf last, `ALLOWED_GLOBS == expected_paths`, 10,000-byte ≈ 2,500-token warning) | PRESENT | `harness/task-lint.sh:9-20,120-123,155` |
| Exact paths load-bearing in the compiled task; path-light only upstream | PRESENT | D11, D23 — already the rule; nothing to add |
| Full spec never in executor context; protocol split from task | PRESENT | D2 |
| Executor never re-decides decomposition; wrong slice → `NEEDS_REPLAN` | PRESENT | `AGENTS.md` Stop conditions |
| DAG metadata kept out of agent-visible frontmatter | PRESENT | D11 ("DAG metadata belongs to the orchestrator") |
| `decisions.md` as a separate planner-owned file | PARTIAL | pgtui tasks ship it (`experiments/tasks/TASK-10x/decisions.md`, global D-ids) but the reference template has no contract for it and no decision carries provenance |
| Binding sources vs orientation hints | PARTIAL | HEAD labels the whole "Read before editing" list non-normative (`reference/task-template/README.md:31`) while `TASK-101/README.md:33` and `harness/testdata/example/README.md:37` put binding decision records in it ("decided, do not reopen"). Ambiguity is real |
| Demo path + independent value | PARTIAL | Goal sentences in pgtui tasks already read as demo paths (TASK-101: "Running `pgtui` opens a TUI … exits cleanly with `q`"), but nothing requires it and the linter cannot see it |
| Verification seam selected before prose | PARTIAL | `notes/example-app-testing.md` chose the pgtui seams (trusted `tests/*`, testcontainers, insta) at planning time; the task itself never states which seam is the acceptance seam or why, so a lower-level substitute test is not detectable |
| AC class + baseline/final polarity per criterion | NEW | D13 proves the whole gate FAIL→PASS; nothing checks each `AC-*` individually. Current AC table is four columns |
| Decision provenance column | NEW | `D-*` entries cite nothing in template, example, or pgtui `decisions.md` |
| `shape` field (tracer-bullet / prefactor / expand / migrate / contract / integration) | NEW | `kind` exists (bugfix…docs) — orthogonal axis |
| Bounded expand→migrate→contract exception | NEW | Conflicts with template `R-004` as written; needs graph-level closure |
| `task-graph.yaml` (blocked_by, frontier, requirement ownership, author-gate status) | NEW | Only representation today: linear ASCII chain in `notes/example-app-decomposition.md` §2 plus "baseline(N+1) = reference(N)" fixture chaining (§5b) |
| Decomposition author gate before `task-lint.sh` | NEW | `harness/README.md` author checklist validates a compiled package; nothing reviews whether the slice is right |
| `graph-lint.sh` | NEW | — |
| Prefactor rule (own task only with independent structural proof + named unblocked tasks) | NEW | — |
| Out-of-scope must name real refused adjacent work, not only boilerplate | NEW | Template placeholder already suggests both kinds; not checked |
| Lifecycle states spec → ticket → package; only leaf packages dispatchable | NEW | — |
| Durable-learning promotion after a passing run (ADR/CONTEXT), never retro-editing the task | NEW | — |
| Ablations A1–A5 and decomposition experiment B | NEW | Backlog candidates (§4 below) |

## 2. Stale or wrong against HEAD

- `raw/03-aihero-attachments/TASK-FORMAT-README-v3.1-EXPERIMENTAL.md` frontmatter has `protected_paths`; removed in `d91e6c0` (D23). Any v3.1 experiment must start from the HEAD template (`expected_paths` only).
- "verifier validates protected-file hashes", "`/task/protected.sha256`", "protected by the manifest", author-gate item "Protected manifest is generated": manifest, `manifest.sh`, `hash-protected.sh` and the hash check no longer exist. Integrity of `/task` is the read-only mount; planner files in `/work` are protected by being outside the whitelist.
- "trusted tests are protected": still true, but by whitelist exclusion, not by hashing.
- Author-gate item "The launch prompt references exactly one task": already the case (`harness/goal-prompt.md`), not a gap.
- Comparison §1 lists `verify.config` fields including "protected" semantics; HEAD `verify.config` has `ALLOWED_GLOBS`, `FORBIDDEN_PATHS`, `REQUIRED_PATHS`, `FORBIDDEN_PATTERNS`, command arrays, `EXTRA_CHECKS` only.
- Everything else cited about the linter, package layout, oracle rule and pgtui chaining matches HEAD.

## 3. Proposed changes (hypotheses, not adopted)

### 3.1 Template (`README.md`) — candidate `task/v3.1`

Each is an independent ablation; adopt one at a time per the §4 plan. Target: stay under the 10,000-byte lint warning (attached v3.1 draft is 7.0 KB).

1. **AC class + baseline/final polarity** (highest value, lowest context cost):

   ```markdown
   | ID | Class | Given / When / Then | Evidence command | Baseline expected | Final expected |
   ```

   `delta` = false/old at `baseline`, true finally; `invariant` = passes at both; `removal` = present at `baseline`, absent finally. Checklist leaf `1.2` records the baseline observation for every criterion, not only the one baseline command. Harness corollary (extends D13 mechanically): the oracle step runs every `AC-*` evidence command on the untouched fixture and on the reference solution and compares to the declared columns — a criterion whose baseline observation does not match its class is rejected before dispatch. This also feeds `ac_coverage` (§6) with per-criterion truth instead of "command executed".
2. **Demo path + independent value** under Goal — two lines: `<command or interaction> → <observable result>` and what remains useful if no later task lands. Lint: presence only; the author gate (§3.3) judges content.
3. **Primary verification seam / why / supporting seams / prior art** under Context. Protocol rule to pair with it: the executor may add lower tests but may not replace the agreed acceptance seam. Observable in metrics as "lower-seam test substitution".
4. **Decision provenance**: `| ID | Decision | Provenance |` inline, or a provenance field per D-id in `/task/decisions.md`. Authoring rule: a compiler imports decisions and never invents one to fill the table; a missing decision is a planning defect, not executor discretion (already the `NEEDS_REPLAN` rule from the executor side; this adds the planner side).
5. **Binding sources vs orientation hints**: split the "Read before editing" list into `Binding sources (conflict → NEEDS_REPLAN)` and `Orientation hints (non-normative)`. Fixes the ambiguity in TASK-101 and the lint example. Cheapest change; can ship with any variant.
6. **`decisions.md` contract** in the reference template: planner-owned, read-only under `/task`, global stable D-ids, provenance per entry, README imports only the D-ids it needs. Today this file exists in pgtui packages without a definition.
7. **`shape` frontmatter** and Scope line "Delivery shape": distinguishes a tracer bullet from a prefactor or a bounded transition role. Only meaningful once a graph exists; P1.
8. **`R-004` bounded exception**: keep "final design directly" as default; allow `expand|migrate|contract` role only when the graph names the sequence and the mandatory contract task. P1, blocked on the graph.

### 3.2 Planner/harness sidecar — `task-graph.yaml`

Not mounted into `/task` (D11 stands). Fields per task: `shape`, `delivers`, `demo_path`, `primary_seam`, `blocked_by`, `owns_source_ids`, `package`, `expected_context`, `transition{mode,role,sequence_id,mandatory_contract_task}`, `author_gate{status,reviewed_by,reviewed_at_utc}`. Invariants (`graph-lint.sh`): acyclic; blockers exist; READY = author gate approved ∧ all blockers complete; one demo path and one primary seam per task; no accidental overlap of `owns_source_ids`; expand-contract sequences close with exactly one contract task blocked by every migrate; initiative completion requires every contract task; packages compiled only after graph approval. Dispatch (`run-headed.sh`) refuses a task that is not READY. Template: `raw/03-aihero-attachments/TASK-GRAPH-TEMPLATE-v1.yaml`. For pgtui the graph is the linear chain 101→106 with `blocked_by: [previous]`.

### 3.3 Author gate before `task-lint.sh`

Operator-reviewed checklist per task (template: `raw/03-aihero-attachments/TASK-AUTHOR-GATE-v1.md`, minus the stale manifest item): decision integrity (provenance, no invented assertions, ADR/glossary/tracker overlap checked, binding vs hint split), boundary (one demoable outcome, owns every layer its ACs need, no AC waits on an unfinished task, fits one context, merge/split considered), dependencies (genuine blockers, no hidden ones, prefactor names what it unblocks, no bookkeeping tickets), seams (chosen first, existing, highest stable, supporting seams justified, prior art named), falsifiability (every AC classed and baseline-checked at the exact base commit), transition policy (direct by default; exception fully closed in the graph), package readiness (lint pass, generated progress, oracle proof). The linter validates a compiled package; this gate validates the slice.

### 3.4 Lifecycle and learning

- States: spec `approved-for-decomposition` → ticket `ready-for-compilation` → package `ready-for-agent`. Only leaf packages are dispatchable; a parent spec must never carry an agent-ready label (AIHero rough edge: an AFK agent picks up the whole spec).
- Tracker text is never the sole execution source; the compiled package is a local read-only snapshot (already D14's spirit; the new part is the rule for the upstream tracker).
- After a passing run: promote durable discoveries to ADR/CONTEXT through a separate reviewed change; never edit the finished task retroactively; re-run the author gate on not-yet-dispatched packages the discovery affects.

## 4. Backlog candidates (for RESEARCH-FINDINGS §7)

Execution-template ablations, one variable each, same fixture/model/effort/seeds/verifier:

| Variant | Change vs `task/v3` |
| --- | --- |
| A1 | + demo path / independent value |
| A2 | + primary/supporting seams |
| A3 | + AC class + baseline/final polarity (+ harness per-AC oracle) |
| A4 | + decision provenance + binding/hint split |
| A5 | all of the above |

Added metrics beyond §6: demo-path execution rate; lower-seam test substitution; per-AC baseline mismatch caught at dispatch; false `NEEDS_REPLAN` rate (a variant that raises replans on satisfiable tasks is not more predictable).

Decomposition experiment (planner side, separate from the execution runs): same settled multi-feature spec to several fresh planner contexts under (1) current authoring, (2) tracer-bullet rule only, (3) + demo path, (4) + blocking edges, (5) full author gate + graph lint. Blind scoring: % tasks with an independent demo, % layer-only tasks, ACs depending on future tasks, blocker precision/recall, cycles/hidden deps, duplicate requirement ownership, context-fit, over-decomposition, prefactor usefulness, expand-contract closure, total compiled token size.

Adoption rule: a change is adopted only if it raises `gate_pass` or review quality without a material rise in context size, turns, or false `NEEDS_REPLAN`. Subjective template quality alone does not promote `task/v3.1`.

## 5. Not borrowed (new reasons only)

- Long numbered user-story catalogue from `/to-spec`: duplicates ACs, adds tokens, poor for refactors/removals (the skill's own docs say so). One-sentence Goal + optional independent-value line is the cap.
- Expand–contract as a general preference: stays a graph-level exception (§3.1 item 8); the compatibility-layer prohibition (`R-004`, D18) remains the default.
- Migration batches that cannot stay green alone need an initiative-level integration harness; never weaken a per-task `verify.sh` to accommodate them.
