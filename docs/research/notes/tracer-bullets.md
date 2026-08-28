# Tracer Bullets + AIHero `/implement` vs `task-format` — what is new (delta note)

Research note, 2026-08-28. Input: `docs/research/raw/04-tracer-bullets.md` (verbatim research reply). Sources it cites, all fetched 2026-08-28 (§3): https://www.aihero.dev/tracer-bullets, https://www.aihero.dev/skills-implement, https://pragprog.com/tips/, https://developers.openai.com/cookbook/examples/codex/using_goals_in_codex, https://docs.anthropic.com/en/docs/claude-code/goal (301 → https://code.claude.com/docs/en/goal, already [C1]), https://x.ai/news/introducing-goal.

The research analysed the v3 package before `d91e6c0` (D23) and before research 03 was stored (`55d1b72`). HEAD is `55d1b72`. This note keeps only what `RESEARCH-FINDINGS.md`, the v3 package and `notes/aihero-spec-tickets.md` (research 03) do not already contain. Research 03 already covers seams, `shape`, demo path, author gate, per-AC polarity and the expand→migrate→contract exception; where research 04 restates those, this note references the 03 section and records only the reconciliation. Nothing here is adopted into `task/v3`.

Verdict carried over unchanged: research 03 fixed the *decomposition* (Level A: one task = one vertical slice). Research 04 adds the missing *execution order inside one task* (Level B): the executor must make one end-to-end path green at its agreed seam before hardening, and the package must make that order mechanical rather than advisory. The outer contract (read-only package, generated progress, verifier, D13 oracle, host rerun) stays the completion authority.

---

## 1. Delta matrix

| Research proposal | Status vs HEAD | Where / what is missing |
| --- | --- | --- |
| Level A: one task = one independently verifiable slice, fresh context per task | PRESENT / NOTE 03 §3.1–3.3 | §1 architecture line, D2, D14; demo path + verticality gate in note 03 |
| Executor never redesigns the slice; false assumption → `NEEDS_REPLAN` | PRESENT | `AGENTS.md:66` |
| Contract read-only, progress separate, verifier + oracle + host rerun | PRESENT | D1, D12–D14 |
| Iteration policy is the one Codex goal element the package lacks | PARTIAL | §2 Codex lists the six elements; `AGENTS.md:22` gives only "ID order unless README states a dependency order" (D17) — an ordering rule, not a feedback policy |
| Level B: tracer-first execution order (RED → GREEN at primary seam → harden → integrated gate) | NEW | Template checklist `README.md:97-113` is baseline → implementation units → AC proofs → gate; nothing names a first end-to-end checkpoint |
| Tracer ≠ prototype (tracer is kept production code; prototype belongs upstream, never inside `/goal`) | NEW | Not stated anywhere; matters because the AIHero article ties tracer bullets to `/prototype` throwaway code (§3) |
| `## Tracer bullet` section (primary path, primary seam, `AC-001` pointer, RED/GREEN expectations, learning boundary, plan-invalidating observations) | NEW, merges NOTE 03 §3.1 items 2–3 | Note 03 put seam/why/supporting/prior-art under Context and demo path under Goal; research 04 folds both plus RED/GREEN and stop triggers into one section. See §4.1 for the merged shape |
| `AC-001` reserved for the primary tracer; baseline command ≡ `AC-001` evidence command | NEW | Template `README.md:39-45` baseline is any command; TASK-101 baseline is `store_test` (a layer), not the goal path |
| Seam column in the AC table | SUPERSEDED by merge | Seam is named once in the tracer section; per-row seam adds a column with mostly repeated values (§4.1) |
| One owning checklist leaf per AC; no duplicated proof leaves | NEW | Template and TASK-101 run the same command under "implemented" and again under "proven" (§2 row i) |
| "Primary feedback loop" protocol invariant (make `AC-001` green before `AC-002+`; preserve it after every later leaf) | NEW | Would be the first `AGENTS.md` change since D2; keep to one short block (§4.3) |
| `PRIMARY_CMD` in `verify.config`, run first; dispatch proves it RED on baseline / GREEN on reference | NEW, extends D13 | `verify.sh:150-158` order is scope → paths → patterns → focused → regression → lint → progress; `FOCUSED_CMDS[0]` is not distinguished |
| Planner-supplied primary test outside `expected_paths` | PRESENT | §5b protection model: all trusted `tests/*` outside the whitelist. Research 04 restates it; nothing to add |
| Lint hard checks for tracer shape (12) + warning heuristics + 8-question author review | NEW / NOTE 03 §3.3 | Hard checks listed in §4.4 are lintable; the 8 questions duplicate the note-03 author gate boundary/seam sections and are folded there |
| `execution_shape: tracer-bullet \| atomic-change` | RECONCILE with NOTE 03 §3.1 item 7 | Note 03 proposed six `shape` values; §4.2 picks the two-value set |
| Unattended feedback: trusted command output replaces the human "get feedback" step | NEW (rationale only) | Follows from [C1]/[O1] evidence model; no artifact change beyond the protocol block |
| Claude: `PRIMARY_CMD` output must be in the transcript; no new `PHASE:` field | PRESENT by construction | D7 turn line + evidence-in-transcript rule; primary-green milestone derivable from the fixed leaf ID |
| Codex: headed launcher uses native `/goal`; `goal-prompt.md` still documents `codex exec` | TRUE, doc defect | `harness/run-headed.sh:123-124,134` (`goals = true`, TUI) vs `harness/goal-prompt.md:32-42` (`codex exec` retry/resume). `status.sh:7` detects Claude `goal_status` only |
| Grok Build adapter; native checklist non-authoritative | NEW | Grok absent from every HEAD document; launcher restricted to `claude\|codex` (`run-headed.sh:29`) |
| TASK-101 rewrite around one seeded-store → render → `q` tracer | NEW (evidence §5) | — |
| TASK-106 split into three tasks | NEW (evidence §5) | — |
| D24 draft | NOT ADOPTED | Text kept in §6 as the candidate decision for after experiment 12 |
| Experiment A (execution order H vs T), Experiment B (task boundary), metric "unverified work before first end-to-end green" | NEW | Backlog §6; distinct from note-03 A1–A5 (those add fields; this changes order and adds a verifier stage) |

## 2. Claim verification against HEAD

| # | Research claim | Verdict | Evidence |
| --- | --- | --- | --- |
| a | TASK-101 checklist is layer-ordered with no single end-to-end command | TRUE | `experiments/tasks/TASK-101/README.md:113-122`: 2.1 store, 2.2 app/keys, 2.3 render, 2.4 CLI, 2.5 unit tests. Goal (`:17`) is launch → list → `q`, but no AC row (`:85-90`) exercises that path in one command; `cli_test` (AC-006) covers exit codes and `--version` only. Fixture is unbuilt at HEAD (`harness/README.md:81`), so trusted test bodies cannot be inspected |
| b | TASK-101 carries insert / duplicate-name / `created_at` work needed only by TASK-102 | PARTIAL | `DuplicateName` and RFC 3339 `created_at` are 101 requirements (`README.md:60,73`, AC-002 `:86`, leaves 2.1.2–2.1.3) and TASK-102 consumes them frozen (`TASK-102/README.md:39,100`). `insert` itself is needed at 101 to seed the two-row list test (AC-004) through the `temp_store` helper, so only the duplicate-name error and the timestamp contract are 102-only |
| c | TASK-106 combines disconnect, terminal restore, gallery | TRUE | `TASK-106/README.md:4,24`; leaves 2.1–2.2 disconnect, 2.3 terminal/exit, 2.4–2.5 gallery + docs |
| d | `goal-prompt.md` documents Codex as `codex exec` retry/resume, not native headed `/goal` | TRUE | `harness/goal-prompt.md:32-42`; launcher already runs the TUI with `goals = true` (`run-headed.sh:118-134`) |
| e | Launcher accepts only `claude\|codex` | TRUE | `run-headed.sh:29` |
| f | Protocol says leaves in numeric order | TRUE, with escape hatch | `AGENTS.md:22` "in ID order unless `README.md` states a different dependency order" (D17). A tracer order can already be imposed per task; nothing requires it |
| g | "Vertical slice" is advisory only | TRUE | `reference/task-template/README.md:58` placeholder; `task-lint.sh` has no check for it |
| h | `verify.sh` has no primary command; check order | TRUE | `verify.sh:150-158`; `verify.config:7` `FOCUSED_CMDS` is an unordered list of per-AC commands |
| i | AC commands duplicated between implementation and proof leaves | TRUE | TASK-101 2.1 ↔ 3.1 (`store_test`), 2.2 ↔ 3.3 (`app_connection_list_test`), 2.3 ↔ 3.2 (`screen_connection_list_test`), 2.4 ↔ 3.4 (`cli_test`); template 2.1.2 ↔ 3.1 |

Stale against HEAD: the research's "path whitelist enforced by the host gate" and "planner-supplied test outside `expected_paths`" already describe D23 (fine); its `TASK-101` summary quotes the decomposition note's `protected_paths` line, which `d91e6c0` removed from the packages. No other stale claims found.

## 3. Vendor and source claims (fetched 2026-08-28)

| Claim | Verdict | Decisive line |
| --- | --- | --- |
| Codex Goals from 0.128.0 | VERIFIED, new to HEAD | "Goals are available starting in Codex 0.128.0." [O1] |
| Codex `/goal pause` / `resume` | VERIFIED, already HEAD | `notes/external-evidence.md:118` |
| Six goal elements incl. iteration policy | VERIFIED, already HEAD | §2 Codex; `external-evidence.md:114` |
| Goals in `codex exec` | NOT ON PAGE | Cookbook page does not mention `codex exec`; HEAD's "not first-class in exec" rests on the maintainer discussion [O1], unchanged |
| Claude `/goal` evaluator sees only the conversation, no commands/files; 4,000 chars; "or stop after 20 turns" | VERIFIED, already HEAD [C1] | "It doesn't run commands or read files independently" |
| Grok Build `/goal` runs to completion + verification, plans, builds its own checklist | VERIFIED, new to HEAD | "It plans an approach, breaks the work into a progress checklist, and starts executing." — no limits documented [H6] |
| AIHero tracer definition, horizontal-layer failure, fresh context per slice, build/test/feedback loop, "Reveal in File System" example, association with `/prototype` throwaway code | VERIFIED | "a small, end-to-end slice of functionality that touches all the layers of your system at once" [H3] |
| `/implement`: five beats, "one tracer-bullet ticket per fresh context window", rough edges (seams not confirmed, no completion step, review output not acted on, "a horizontally-layered ticket gets built as written") | VERIFIED | [H4] |
| Pragmatic tips: #20 tracer bullets, #21 "Prototyping is a learning experience. Its value lies not in the code you produce, but in the lessons you learn.", #42 small steps | VERIFIED | [H5]. The "tracer code is kept, prototype code is thrown away" contrast is the book's; the tips page states only the prototype half |

## 4. Proposed changes (hypotheses, not adopted)

### 4.1 One `## Tracer bullet` section (merges note 03 §3.1 items 2–3 with research 04 §7)

Placed after Context, before Preconditions. Replaces the separate "Demo path / Independent value" lines under Goal and the "Primary verification seam / why / supporting / prior art" block under Context proposed in note 03; one block, one lint target:

```markdown
## Tracer bullet

Primary path: `<trigger>` → `<public entry point>` → `<system path>` → `<observable result>`.
Independent value: <what is usable after this task alone>.
Primary seam: `<highest stable public boundary>` — why: <proves the outcome without coupling to internals>.
Supporting seams: `none` | <seam — invariant the primary seam cannot observe>. Prior art: `<existing test>`.
Primary criterion: `AC-001` — evidence `<command>`; baseline `<RED result>`; final `<GREEN result>`.
Learning boundary: make `AC-001` green before `AC-002+`; tracer code is final code, not scaffolding; no adjacent entry points or variants before green.
Plan-invalidating: <observation that proves the seam or a fixed decision wrong> → `NEEDS_REPLAN`.
```

Rules: the Baseline command under Context is the `AC-001` evidence command (or its filtered form); `AC-001` is the tracer; `AC-002+` harden the same capability; a criterion with a different actor, entry point or terminal outcome belongs to another task. AC table keeps the note-03 six columns (`ID | Class | Given/When/Then | Evidence | Baseline expected | Final expected`); `AC-001` is always `delta`; no per-row seam column.

### 4.2 `shape` reconciliation

Note 03 proposed `shape: tracer-bullet | prefactor | wide-refactor-expand | wide-refactor-migrate | wide-refactor-contract | integration`; research 04 proposes `tracer-bullet | atomic-change`. Take the two-value set, field name `shape` (D11: every frontmatter field must have an in-container consumer — here the linter and the protocol block): `tracer-bullet` (default for `kind: feature|bugfix`) and `atomic-change` (narrow refactor, removal, docs, mechanical test change: primary invariant + postcondition instead of a tracer). The four transition values are meaningful only once `task-graph.yaml` exists and stay in note 03 §3.2 as graph-level roles; `prefactor` is an `atomic-change` whose graph edge names what it unblocks.

### 4.3 Checklist skeleton and protocol block

Skeleton: **1** primary target established (preconditions; `AC-001` RED recorded in `BASELINE:`) → **2** primary tracer GREEN (`2.1` smallest end-to-end path passes `AC-001`; `2.2` diff contains only what `AC-001` and the fixed decisions require) → **3** same capability hardened (one leaf per remaining AC; each AC has exactly one owning leaf) → **4** integrated (all AC commands together with `AC-001` still green; scope diff; `verify.sh` DONE). Removes the duplicated "proven" leaves (§2 row i): the first milestone becomes "the declared path works at the agreed seam", not "a layer compiles".

Protocol addition to `AGENTS.md` (the iteration policy; one block, ~70 words): for `shape: tracer-bullet`, after recording the `AC-001` baseline work only on the smallest production-quality path that makes the exact `AC-001` command pass; do not start `AC-002+`, cleanup or adjacent variants before it is green; keep it green after every later leaf; if the declared seam, required path, fixed decision or expected baseline is materially wrong stop with `NEEDS_REPLAN`. Do not paste the article — advisory prose is what the current "vertical slice" placeholder already is.

### 4.4 Verifier and linter

- `verify.config`: `PRIMARY_CMD="<AC-001 command>"`; `verify.sh` runs `CHECK primary` immediately after the integrity checks and before `focused`. Dispatch oracle (D13 extended): `PRIMARY_CMD` must fail on the untouched fixture for the declared reason and pass on the reference; the whole gate keeps the same rule. Complements note 03's per-AC polarity oracle (that one runs every AC; this one names which AC is the tracer).
- `task-lint.sh` hard checks for `shape: tracer-bullet`: section present in position; `AC-001` exists and is `delta`; Baseline command == `AC-001` command == `PRIMARY_CMD`; the first implementation leaf references `AC-001`; no `AC-002+` leaf precedes the primary-green leaf; every AC has exactly one owning leaf; the integrated leaf reruns `AC-001`; `kind: feature|bugfix` without `shape: tracer-bullet` requires a declared exception; `atomic-change` names a primary invariant. Warnings: title joins outcomes with "and"; implementation parents named after layers (`store`, `backend`, `UI`, `CLI`…); `expected_paths` spanning unrelated subsystems; no evidence command touching the public entry point. Author judgment (verticality, highest seam, fits one context) stays in the note-03 author gate.

### 4.5 Adapters

- Codex: `goal-prompt.md` needs a headed `/goal` section mirroring `run-headed.sh` (`goals = true`, TUI, same injected condition); keep `codex exec` as a headless fallback only. `status.sh` needs Codex completion states (goal active / complete / blocked / idle) — today only Claude's transcript `goal_status` is authoritative (`status.sh:7`).
- Grok Build (`[H6]`): adapter + image behind the same task schema; launch prompt states that the `/task/README.md` checklist mirrored in `/progress/progress.md` is authoritative and the native goal checklist is a projection that may not add, remove or reorder leaves; completion is still the host verifier. New metric: native goal "complete" ∧ `¬gate_pass` (a `false_done` variant keyed on the vendor's evaluator instead of the agent's report).

## 5. Experiment packages — concrete findings

- **TASK-101.** Goal is a tracer (launch → stored row rendered → `q` → exit 0); no criterion proves it in one command; `AC-001` and the baseline are `store_test`, a layer. A tracer variant needs one planner-shipped end-to-end test (e.g. `tracer_connection_list_test`: seeded store → `App` init → first frame → `q` → `Effect::Quit` → terminal teardown) as `AC-001`/`PRIMARY_CMD`/baseline, then empty state, sort, `j`/`k` clamp, unwritable `--db`, `Ctrl+C` as hardening. Move `DuplicateName` and the `created_at` contract to TASK-102 (which owns the create path); `insert` stays because the list test seeds through it.
- **TASK-106.** Three finish lines (disconnect, exit/terminal, gallery) with different triggers and seams. Candidate split: disconnect returns to the list (tracer: `d` → `Effect::Disconnect` → `Msg::Disconnected` → list rendered), exit restores the terminal from every state (`atomic-change`, invariant), gallery renders every screen (`atomic-change`, artifact). This is Experiment B's natural fixture: one broad task vs three packages with fresh contexts.
- Both changes are planner work on `experiments/tasks/`; they must not land before Experiment 12 has a control run of the current packages (§7 item 1 still open).

## 6. Backlog candidates (for RESEARCH-FINDINGS §7)

- **Experiment 12 — execution order (Level B), same scope and tests:** H = current layer-ordered checklist; T = tracer section + `AC-001` primary + `PRIMARY_CMD` + tracer-first skeleton + protocol block. Fixtures: one cross-layer feature (TASK-101 tracer variant), one bugfix (the expired-token lint example), one removal/refactor as `atomic-change` control. New metrics: turns/tool calls until primary green; files and lines changed before primary green ("unverified work before first end-to-end green" — the tracer-specific metric); lines rewritten or deleted after primary green (rework); primary-green leaf claimed while `PRIMARY_CMD` fails (harness reruns it); variance across seeds of the above.
- **Experiment 13 — task boundary (Level A):** one broad package (TASK-106 as is) vs three one-tracer packages with fresh contexts; measure `gate_pass`, total tokens/cost for the whole feature, false_done, `NEEDS_REPLAN` rate, compactions. Separates decomposition from ordering.
- **Candidate decision D24 (adopt only after 12):** feature and bugfix packages contain one primary tracer bullet; `AC-001` is its pre-agreed public seam; the executor records `AC-001` RED then makes it GREEN before later criteria; independent behavior is a new task and a fresh context; a false seam or fixed assumption is `NEEDS_REPLAN`; tracer code is production code, prototypes happen upstream.
- Doc/harness follow-ups independent of experiments: `goal-prompt.md` headed-Codex section; `status.sh` Codex states; Grok adapter after the first Claude and Codex runs.

Adoption rule as in note 03 §4: T replaces H only if `gate_pass` rises or unverified-work-before-green and rework fall without a material rise in context size, turns or false `NEEDS_REPLAN`.

## 7. Not borrowed (new reasons only)

- Tracer bullet ≠ prototype: the AIHero article ties tracer bullets to `/prototype` throwaway code; inside `/goal` the tracer is the final implementation (Pragmatic tip #21 keeps prototypes as learning, not code). Prototyping happens upstream, before decisions are fixed; an executor that discovers a false assumption stops with `NEEDS_REPLAN` instead of experimenting.
- Executor-discovered seams: `/implement` assumes seams were agreed upstream and does not confirm them; the seam is named in the package (§4.1), never chosen during the run.
- Commit as completion, same-agent review as verification, unchecked tracker state, one shared checkout for concurrent agents: all weaker than the host verifier, the fresh-reviewer backlog item (§7 item 7), the read-only package and the per-run `/work` copy.
- Accepting a horizontal ticket and hoping the executor reshapes it: `/implement` builds it as written; `task-format` rejects it at lint (§4.4) and at the author gate (note 03 §3.3).
- A vendor's native goal checklist as a source of truth (Grok): a third checklist beside `README.md` and `progress.md`; it is a projection only.
