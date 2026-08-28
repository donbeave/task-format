# Synthesis 2026-08-28 — research 03/04 + independent survey → v3.1 changes

## Port to task/v4 (2026-08-29)

`main` moved to `task/v4` (commit `470f009`: Rust `taskfmt` harness, empty-repo lifecycle, TASK-001..007, decisions D24–D30 in `RESEARCH-FINDINGS.md`) while the v3.1 work below was in flight on branch `research-improvements`. The v3.1 bash implementation is preserved there unchanged (commits `55d1b72` research 03, `67d4949` research 04, `dd82f11` implementation, `3dff488` this synthesis). This branch (`port/v4-research-improvements`) cherry-picks the two docs-only research commits and ports the **decisions** — renumbered **D31–D39** in `RESEARCH-FINDINGS.md` §3 — onto the Rust harness; the bash diff itself is not merged. Schema string stays `task/v4`.

Numbering: v3.1 D24–D32 in this note ≡ D31–D39 in the findings (D24 → D31 two-phase gate, D25 → D32 INCOMPLETE, D26 → D33 scope fail-closed + base SHA, D27 → D34 proof leaves, D28 → D35 hints/decisions.md, D29 → D36 selfcheck, D30 → D37 lint C1–C9, D31 → D38 protocol polish, D32 → D39 Codex headed dispatch). The findings' D24–D30 are the v4 decisions (Rust CLI, trusted base commit, `--auto`, profiles, TASK-001..007, `verify.toml`, preload).

Artifact mapping (every script name in §1–§11 reads through this table):

| v3.1 artifact (branch `research-improvements`) | v4 equivalent (this branch) |
| --- | --- |
| `reference/task-template/verify.sh` | `taskfmt verify` (Rust gate, baked into the run image; same binary on the host) |
| `PROGRESS_FILE= /task/verify.sh` (phase 8a) | `taskfmt verify --progress ""` |
| `verify.config` (`FOCUSED_CMDS`, `REGRESSION_CMDS`, `LINT_CMDS`, `ALLOWED_GLOBS`, `FORBIDDEN_PATTERNS`, `BASE_REF`) | `verify.toml` (`[focused]`/`[regression]`/`[lint]` `commands`, `allowed_globs`, `[[forbidden_patterns]]`, `forbidden_paths`, `required_paths`, `base_ref`) |
| `EXTRA_CHECKS` bash hooks | dropped (D29); host-only `verify.toml` overlay is backlog 19 |
| `harness/task-lint.sh` (C1–C9) | `taskfmt lint`; corpus `harness/tests/lint_corpus.rs` |
| `harness/progress-init.sh` | `taskfmt progress-init` |
| `harness/selftest.sh` (56 scenarios) | `taskfmt selftest` + `cargo test` (`harness/tests/gate_tamper_matrix.rs`, `lint_corpus.rs`) |
| `harness/gate-selfcheck.sh <task> <fixture> <reference\|.patch>` (nop / polarity / oracle) | `taskfmt selfcheck <task> <workspace>` (shipped; opt-in at dispatch via `--selfcheck`): nop + focused-only polarity against the trusted base commit (regression INFO, unrunnable = NOVERDICT); oracle only when a reference is supplied — none ship yet |
| `run-headed.sh` writing `baseline_sha` to `meta.json` | `taskfmt run` records `manifest.json base_sha` (D25) and passes it into the container as `TASKFMT_BASE` (was the movable `baseline` tag) |
| `VERIFY_BASE_REF=<sha>` host override | `taskfmt verify --base <sha>` / `TASKFMT_BASE`; `taskfmt gate <run>` sets it from the manifest |
| `status.sh` (`goal_verdicts: null` for Codex, `baseline_sha`, `baseline_tag_ok`) | `taskfmt status` (`goal_verdicts` null for codex, `transcript: n/a`, `base_tag_ok`, `report_status`) |
| `goal-prompt.md` per-agent fence tag ` ```text claude codex ` | same file, block selected by the profile's `kind` |
| duplicate Codex `config.toml` heredoc removed ("single config source") | moot — v4 preseeds `config.toml`/`settings.json` only from `harness/src/ops/container.rs` |
| `harness/README.md` author checklist rules | same file (v4 crate README) |
| `notes/example-app-decomposition.md`, `notes/headed-herdr-harness.md` banners | applied verbatim |
| TASK-101..106 package fixes (F1–F5, E7) | TASK-001..007 analogues (002 store+list, 003 create form, 004 connect+sidebar, 005 preview+sort, 006 custom SQL, 007 disconnect/exit+gallery): F1 ` -- ` filter form → C7 WARN; F2 leaked-container check → a `[regression]` command with the testcontainers label filter; F4 snapshot `check_no_snap_new` → moot (behavioral tests, D28); F5 protected files → `forbidden_paths` (D28, `1042f5e` dir-prefix semantics); F3 fixture chain (`pgtui-10N`) → moot (D25: base = fresh `origin/main` after task N−1) |
| `schema: task/v3` unchanged | `schema: task/v4` unchanged |

### Merge review 2026-08-29

The port (PR #1) went through five independent reviewers and three paired debates before merge. Outcome per topic:

- **T1 selfcheck in dispatch — reviewer Q's structure won, with P's semantic fixes.** Accepted facts: the "regression PASS at base" polarity contradicts D28 (packages ship their own new tests) and would refuse all seven packages; rc 126/127 counted as RED gives a vacuous PASS; host-mode selfcheck is a false RED for postgres-backed focused tests (TASK-004+); the oracle never runs (no references); `experiment --auto` died at TASK-001; `--skip-selfcheck` would have been used every time. Result: polarity = every `[focused]` FAIL at base, `[regression]` INFO only; unrunnable command = NOVERDICT (exit 69, never RED); the dispatch precondition is **opt-in** (`--selfcheck` on `run`/`experiment`, `--skip-selfcheck` removed, manifest `selfcheck: not-run|pass|fail|noverdict`, lint first); TASK-001 `[focused]` #4 (`git diff --exit-code -- render.rs fonts`) dropped — `forbidden_paths` covers it. Backlog: container prereq-stage selfcheck (then default-on), reference solutions.
- **T2 lint — split.** C3 stays ERROR without new syntax (the 13 hits were real); the author checklist now says behavioral `R-*` on the proving item, policy `R-*` on the diff-review leaf, "cite everything on the gate leaf" is an anti-pattern. C6 reshaped: rows whose command is the frontmatter `verify` skip; `cargo test` compared by parsed target set (package, `--test` targets, filter, trailing args; order-insensitive); exact substring otherwise — 0 warnings expected after TASK-004..007 README/verify.toml order alignment. C9 reshaped: only `kind ∈ {feature, bugfix}`; tolerant of trailing `-- <filter>` / a subset of `--test` targets; suppressed when the baseline command matches a `verify.toml` command by target set (clears TASK-001).
- **T3 AGENTS.md — reviewer P mostly; cuts agreed by both.** Cut: the `precondition-broken` label (the rc-127-is-BLOCKED sentence stays), the step-1 "(orientation only, not rules)" parenthetical, the INCOMPLETE bullet's duplicated "report `STATUS: INCOMPLETE`" clause. Kept: git-derived `CHANGED:` (the evaluator is transcript-only and gate stdout lists no files on PASS), the behavior-scope prohibition, the flaky-check rule, step-7 parent evidence.
- **Consensus corrections:** `lint.rs` loses the `EMBEDDED_TEMPLATE` `include_str!` fallback (the template is the research variable; hard error instead); `status.rs` `base_tag_ok` returns `None` on a git error (was `Some(false)`); crate version bumped + README note that the in-container `taskfmt` is the image copy (rebuild after upgrading; runtime version check → backlog); the tracked-whitelisted-`.gitignore` scope hole → backlog with the exact fix; TASK-004..007 README regression commands aligned with `verify.toml` target order; findings D33/D36/D38 text corrected (see below); docs stay in this PR.

D-number remap (this note → findings on `main`): D24 → D31, D25 → D32, D26 → D33, D27 → D34, D28 → D35, D29 → D36, D30 → D37, D31 → D38, D32 → D39. Rows in §3 that say "fixed (D24)" etc. use this note's v3.1 numbering.

Sections §1–§10 below are kept as history of the v3.1 branch; script names are left as written there. §11 is replaced by the v4 verification block.

Status: research synthesis and change record for the branch `research-improvements`.
Inputs: `raw/03` (AIHero `/to-spec`, `/to-tickets`), `raw/04` (Tracer Bullets, AIHero
`/implement`), `notes/aihero-spec-tickets.md`, `notes/tracer-bullets.md`, eight parallel
sub-agent reports (this session), one skeptic verdict over the consolidated change list.
Decisions recorded here as D24–D32 (v3.1 numbering; = D31–D39 in `RESEARCH-FINDINGS.md` §3,
remap above). Nothing here was run against a live agent; every claim about `/goal` behaviour is a hypothesis until §7 experiments run.

## 1. Method

Eight agents ran in parallel, each with one perspective and no view of the others:

| Agent | Perspective | Input |
| --- | --- | --- |
| R1a | mechanism extraction | research 03 + attachments |
| R1b | skeptic (token weight, duplication, conflicts, failure scenarios) | research 03 |
| R2a | mechanism extraction | research 04 |
| R2b | skeptic | research 04 |
| E1 | external survey: spec/task formats (spec-kit, Kiro/EARS, OpenSpec, BMAD, Cursor, Gemini, Devin, Codex cookbook, 2026 papers) | web |
| E2 | external survey: verification, completion proof, compaction, resumability (SWE-bench grader, Harbor, Claude/Codex/Grok goal docs, mutation testing, Anthropic harness, Ralph) | web |
| C1 | audit of `reference/task-template/` + `harness/` by hypothetical `/goal` walkthrough, with probes against `verify.sh`/`task-lint.sh` | repo |
| C2 | audit of `experiments/tasks/TASK-101..106` + harness consistency; ran lint/selftest | repo |

Their findings were merged into a change list (scratchpad), then a ninth agent with full
context acted as skeptic over every item, verified each cited defect at HEAD, and ruled on the
three disagreements (§6). Implementation ran as parallel agents per file group, followed by a
wiring wave (selftest, lint corpus, packages) and a final review. Rule: adopt (a) verified
defects, (b) zero-agent-token structural changes in lint/verify/harness, (c) executor-visible
text only when a concrete failure mode is shown in the corpus or by probe and net token cost is
≤ 0 or a few words; everything else → §7 backlog.

## 2. What v3 already did well (both auditors: do not touch)

- `README.md` byte-immutable on an `:ro` mount; checkbox state in a generated `progress.md`
  diffed modulo `[ ]/[x]`; selftest proves reword/delete/add/parent-child tampers fail (D1).
- `verify.sh` contract: `CHECK/SUMMARY/RESULT` lines, `DONE` only from `finish`, ERR trap →
  never a silent pass; real rc preserved through subshells (D12).
- Whitelist-only scope, `expected_paths` ≡ `ALLOWED_GLOBS` lint-enforced, untracked files
  counted (D23). In the pgtui corpus every planner test lives outside the whitelist, so
  "lower-seam test substitution" is structurally impossible at the gate — this is stronger
  than any seam prose research 03 proposes.
- Preconditions as commands; baseline command with expected pre-change failure; `BASELINE:`
  gate-checked (D10).
- Checklist lint: contiguity, depth = ID components, 5–20 leaves, no single-child parent, gate
  leaf last, AC coverage. `progress-init.sh` refuses on lint failure; `progress.md` gitignored.
- Protocol/task split; total instruction budget ≈ 2.3k tokens (AGENTS 748 w + README template
  746 w + 110 w condition); six prohibitions, each with a gate-visible symptom (D2).
- Final report grammar ending in one parseable `GOAL_RESULT`; `status.sh` ranks the host gate
  above the transcript verdict.
- The current `/goal` condition already matches the Claude docs pattern (end state + stated
  check + constraints + turn clause) and the Codex six-element goal anatomy minus "iteration
  policy".

## 3. Verified defects found independently by the auditors

C1 (template/protocol/verify/lint), probes run against HEAD:

| # | Defect | Status |
| --- | --- | --- |
| 1 | AGENTS step 8 said "run verify.sh to DONE, *then* set STATE DONE" but `check_progress` needs `STATE: DONE` + every leaf `[x]` incl. the gate leaf → cannot pass; D6 described the fix but it never reached the protocol | fixed (D24) (v3.1 numbering; = D31 on main) |
| 2 | `baseline` is a movable tag in agent-writable `/work`; `git tag -f baseline HEAD` hides every change from the scope check; SHA recorded nowhere | fixed (D26: `baseline_sha` in `meta.json`, `VERIFY_BASE_REF`) (v3.1 numbering; = D33 on main) |
| 3 | `git diff --name-only` with default rename detection: staged `git mv out-of-scope → in-scope` lists only the new path → scope PASS | fixed (`--no-renames`) |
| 4 | `.git/info/exclude` line or `skip-worktree`/`assume-unchanged` flag hides files from the scope check | fixed (`--exclude-per-directory=.gitignore`, hidden-index-entry check) |
| 5 | Commands ran under `bash -o pipefail -c`: `"false; true"` passes | fixed (`-eo pipefail`; all 36 corpus commands unaffected) |
| 6 | No terminal status for the turn/budget cap; agent must misreport as DONE/BLOCKED/NEEDS_REPLAN | fixed (D25 `INCOMPLETE`) |
| 7 | "Read before editing (non-normative hints)" lists binding decision records | fixed (D28) (v3.1 numbering; = D35 on main) |
| 8 | D17 ordering override has no slot in the template | deferred (no evidence a task needs it yet) |
| 9 | BLOCKED/NEEDS_REPLAN boundary overlaps; errored precondition command indistinguishable from false one | fixed (D31) |
| 10 | Resume branch keyed on `STATE: IN_PROGRESS`, which the generator always writes → every fresh run "resumes" | fixed (keyed on `## Log` non-empty) |
| 11 | Handoff `NEXT:` duplicates `CURRENT:` (D4 class) | fixed (dropped) |
| 12 | Leaf 4.1 evidence `git diff --stat` vs index — vacuous once work is committed | fixed (`--no-renames --stat baseline`) |
| 13 | Template claims "verify.sh runs these" but lint never relates AC commands to `verify.config`; `FOCUSED_CMDS=("true")` lints clean | fixed (lint WARN + "or a superset" wording) |
| 14 | Lint accepts: empty `evidence:`, duplicate IDs, uncited `R-*`, leftover `<placeholders>` not in the hand list, `<test for AC-002>` in config, AC cited only on a parent | fixed (D30) |
| 15 | Selftest never exercised an out-of-scope file (`ALLOWED_GLOBS=("*")`) | fixed (9 scope scenarios) |
| 16 | No rule for a flaky check | fixed (D31) |
| 17 | Rules stated 4–5× across README/AGENTS/goal-prompt | partly fixed (README duplicates removed; cross-file repetition kept where the reader differs) |
| 18 | Template AC-003 expected `exit 1` while every other AC is exit-0 | fixed (`! grep …` → exit 0) |
| 19 | Transcript `DONE` forgery fools the Haiku evaluator, not the host gate | cannot fix in prose (§10) |
| 20 | `goal-prompt.md` Codex section documents `codex exec` while dispatch is headed TUI | fixed (D32) |

C2 (experiment packages + harness):

| # | Defect | Status |
| --- | --- | --- |
| F1 | 9 leaves use `cargo test --test x a b` (invalid; one positional) | fixed (` -- `) + lint WARN |
| F2 | TASK-103 `no_leaked_containers` matched the standing `prereq-postgres` by image → gate fails unconditionally | fixed (testcontainers label filter, verified in testcontainers-rs 0.28.0) |
| F3 | `TASK-104/105/106/fixture` pointed at `pgtui-103/104/105` (previous task's baseline) | fixed + fixtures README rows |
| F4 | `check_no_snap_new` `! find … \| grep -q .` can flip under SIGPIPE | fixed (`-print -quit` form) |
| F5 | TASK-101 whitelist covers files its R-007 calls protected | fixed (`protected_untouched` extra check) |
| F6 | 24 identical evidence-command pairs between 2.x implement leaves and 3.x proof leaves across six packages | fixed (D27; section 3 dropped everywhere) |
| F7 | `/task/decisions.md` listed under non-normative hints in all six; AGENTS files table did not name it | fixed (D28) (v3.1 numbering; = D35 on main) |
| F8 | Same `D-id` with different text across tasks (D-024, D-060, D-011/D-033 subsets) despite "means the same thing in every task" | deferred (§8; needs pgtui design judgment) |
| F9 | ACs that pass at baseline (TASK-106 AC-006 invariant, AC-004 likely; TASK-105 AC-002 half); requirements with no AC (102 R-004, 106 R-006, 101 R-005) | AC-006 moved to `REGRESSION_CMDS`; polarity now caught mechanically by `gate-selfcheck.sh` (D29); remaining suspicions in §8 |
| F10 | TASK-101 has no stated home for `DbError`/`ResultSet`/`QueryKind` placeholders → per-seed variance | deferred (§8) |
| F11 | `ui/status.rs` outside 102–105 whitelists although each adds a help string | deferred (§8) |
| F12 | Two authoring generations (101–103 vs 104–106) differ in boilerplate | reduced by the checklist rewrite; residual style drift accepted |
| F13 | TASK-101 implements `insert`/`DuplicateName` with no consumer until 102 | deferred (§8; `insert` seeds AC-004, only `DuplicateName`/`created_at` are 102-only) |
| F14 | Codex path Claude-shaped: same prompt block injected, config defined twice, `status.sh` reports `goal_verdicts: 0` for Codex without parsing anything | fixed (D32) |
| F15 | Gate example path `<run>/progress.md`; stale `protected_paths`/manifest text in two notes | fixed (path) / banner added to both notes |

## 4. Research 03 and 04 — adoption per concept

| Concept | Source | Verdict | Where / why |
| --- | --- | --- | --- |
| Per-AC class + baseline/final polarity | 03 | adopted as **harness oracle**, rejected as README columns | `gate-selfcheck.sh` proves each `FOCUSED_CMD` fails on baseline and passes on reference, each `REGRESSION_CMD` passes on both (D29). README columns rejected: SWE-bench hides F2P/P2P from the agent; corpus is all-delta (constant column); positional lint would break; 6 extra broken-tree compiles per run; false `NEEDS_REPLAN` risk |
| Binding sources vs orientation hints | 03 | adopted, minimal form | hints "orientation only, never `/task/*`" (lint C8); `decisions.md` binding in the AGENTS files table + one line under Fixed decisions (D28). No new section |
| Real refusals in Out of scope | 03 | adopted as author rule | `harness/README.md` author checklist; unlintable |
| Decision provenance | 03 | rejected from README; author rule | executor cannot act on a source it cannot read; "import, never invent" + `decisions.md` one-per-fixture contract in the author checklist |
| Verification seam block (primary/why/supporting/prior art) | 03 | rejected | the AC evidence command *is* the seam; whitelist already forbids substitution; +400 B of prose with no consumer; seam-vs-command disagreement would become a stop |
| Demo path / independent value | 03/04 | rejected as prose; adopted as author rule | "for feature/bugfix, `AC-001` is the Goal path end-to-end and the Baseline command is the AC-001 command" (author checklist + lint C9 WARN). Prose demo line without a command violates D5; a headed TUI demo cannot run in the agent's shell |
| `shape` / delivery-shape / expand-migrate-contract role | 03/04 | rejected (package); §8 (graph) | no in-container consumer (D11); bounded-transition text conflicts with `FORBIDDEN_PATTERNS`; `R-004` already carries the escape "unless a requirement above demands one" |
| Task graph, author gate, graph-lint, lifecycle states | 03 | deferred (§8) | upstream of the package; no runs exist to motivate the build |
| Durable-learning promotion / no retro-edit of finished tasks | 03 | adopted as author rule | zero tokens; protects reproducibility of past runs |
| Tracer-first execution order inside a task | 04 | rejected as protocol text; backlog Exp 12 narrowed | no enforcement channel (verify.sh sees the final tree; Haiku judging order = variance); unsatisfiable on greenfield Rust where `tests/support/mod.rs` compiles against every layer's surface. Metric `changes_before_first_ac1_pass` adopted (§6) |
| `AC-001` reserved as primary tracer, baseline ≡ AC-001 | 04 | adopted as author rule + lint C9 WARN | exact equality would fire on 4/6 packages (baseline = whole file, AC = filter); WARN, not ERROR |
| `## Tracer bullet` section | 04 | rejected | copies Goal + AC-001 row + Baseline block (~150 tokens, fourth copy of one string) |
| `PRIMARY_CMD` first in `verify.sh` | 04 | rejected | verify.sh runs every check; order changes stdout only; a fourth copy of the AC-001 string (D4/D9 drift class); `FOCUSED_CMDS[0]` convention already documented |
| One owning leaf per AC / drop per-AC proof leaves | 04 (+C2 F6) | adopted, modified | section 3 dropped; AC cited on the item whose evidence is its command; **parents with their own `evidence:` must be run** (closes the hole where template parents carried evidence the protocol never executed) (D27) |
| "Primary feedback loop" protocol block (~70 words) | 04 | deferred to Exp 12 | see tracer-first |
| Tracer ≠ prototype | 04 | already covered | `R-004` + NEEDS_REPLAN on false assumption |
| Planner-supplied primary test outside whitelist | 04 | already present | D23/§5b; template comment on `expected_paths` now says so |
| Lint hard checks 1–12 | 04 | 5 decidable adopted (C1, C5, C8, C9, ID checks), 3 fake rejected ("identifies a seam", "transitively includes", "names invariant") | |
| Grok adapter | 04 | deferred (§8) | no verified CLI, limits, transcript, or prompt-constrainable checklist |
| TASK-101 rewrite around a planner-shipped end-to-end test; TASK-106 split | 04 | deferred to Exp 12/13 | control run first; TASK-106-B ("exit restores terminal") may have no RED baseline |
| Codex headed `/goal` doc, status detection | 04 | adopted (D32) | verified defect |

## 5. Independent external findings

| Source (quote) | Mechanism | Verdict |
| --- | --- | --- |
| SWE-bench `grading.py`: "If fail-to-pass (Resolution) = 1 and pass-to-pass (Maintenance) = 1 -> FULL … Otherwise -> NO"; "P2P semantics: a skipped test is not a regression, unlike for F2P"; eval scripts "end with a `git checkout` that resets the test files" [E4] | two-polarity test sets, grader owns them, test files reset before grading | adopted: `gate-selfcheck.sh` polarity (D29); test-file reset unnecessary here (planner tests are outside the whitelist → scope FAIL) |
| Harbor: "The agent does NOT have access to `/tests/` or `/solution/` at runtime"; rubric `functional_verification` ("not source code keywords or string patterns"), `test_instruction_alignment`, `binary_reward`; hack prompt "creating files that match expected patterns without real content" [E5]; 2604.28093 ">15% of tasks were demonstrably reward-hackable" | oracle + nop at review time; adversarial trial | adopted: nop/oracle automation (D29); backlog: cheat-seed variant, hidden host-only checks (`FORBIDDEN_PATTERNS` greps are exactly the "keyword" class Harbor rejects — kept as structural guards, not primary proof) |
| Claude goal docs: "It doesn't run commands or read files independently"; turn clause "judged from the conversation" (model-judged, not enforced); resume "resets the turn count" [C1]; issues #15174 (hook stdout not injected after compaction), #43733 ("hit or miss whether Claude reliably re-reads the right files") [C7] | evaluator sees transcript only; compaction hooks unreliable | adopted: condition "verify.sh run after the last file change"; protocol "after compaction or any resume, re-read README + progress before editing"; D16 hooks stay optional |
| Codex cookbook: "completion must be evidence-based"; budget limit "is not the same as completing the objective… summarize progress and blockers"; blocked stop = "attempted paths, the evidence gathered, the blocker, and the next input needed" [O1] | budget ≠ done; what-was-tried | adopted: `INCOMPLETE` (D25); "report what was tried" in stop conditions (D31) |
| Grok: "plans an approach, breaks the work into a progress checklist" [H6]; community "GOAL.md"/verifier sub-agent framing UNVERIFIED | native third checklist | §8 |
| BMAD #1789: File List "agent-driven… against its own mental model", omits lockfiles → fix "Run `git status --porcelain` and `git diff --name-only`" [X1] | git-derived change list | adopted: `CHANGED:` = verbatim `git diff --no-renames --name-status baseline` + untracked (D31) |
| BMAD #496: `Status` in both allowed and forbidden lists → agent "conservatively follow[s] the prohibitive instruction" [X1] | contradictory permissions freeze completion | negative evidence supporting D23 single-list scope and the audit of README/AGENTS contradictions (§3 #1, #7, #9) |
| BMAD `HALT` on "3 consecutive implementation failures" | numeric spin bound | backlog (false-stop risk unmeasured) |
| spec-kit: `[NEEDS CLARIFICATION]` blocks the plan; Complexity Tracking table "why simpler alternatives were rejected"; "Halt execution if any non-parallel task fails" [X2] | structured deviation record | backlog (`DEVIATIONS:` grammar) |
| Kiro/EARS: "While <precondition>, When <trigger>, the <system> shall <response>" [X3] | requirement grammar forcing trigger/state/error branches | backlog (ablation; authoring cost) |
| OpenSpec `validate --strict`: "requirements missing scenarios" fails [X4] | every requirement has a scenario | adopted: lint C3 (every `R-*` cited by an AC row or checklist item) |
| Gemini CLI plan mode: only read tools + `.md` writes allowed until approval [X5] | tool-level phase gating | supports backlog item 8 (`PreToolUse` deny) |
| Papers: 2604.05278 (validation hooks +1.7 Pass@1), 2601.20404 (AGENTS.md −28.6% runtime, no correctness claim), 2603.05744 ("reproduction steps, expected behaviors, and targeted exploration hints" → +20% resolution), 2603.26233 (clarification scaffold closes the underspecification gap), 2605.30314 (agents find spec gaps ≤ 44.4%) [X6] | — | 2603.05744 is direct evidence for the Baseline + Desired-behavior + read-list blocks; 2605.30314 means author-side lint/oracle must catch spec gaps — the executor will not |
| Mutation testing as completion proof (Test Double; Microsoft testing agent) [X7] | agent-authored test must fail when the change is reverted | backlog: per-AC sensitivity check |
| Anthropic harness: "Claude tended to mark a feature as complete without proper testing"; Ralph: "`--completion-promise` uses exact string matching" [A1] | — | already covered by D1/D12; per-leaf commits as second ledger → backlog |

## 6. Disagreements investigated

**AC polarity — README columns (R1a) vs harness-only oracle (R1b).** Verdict: harness only.
Evidence: SWE-bench keeps F2P/P2P on the grader side; in the pgtui corpus every AC is `delta`
(trusted tests do not compile at baseline) so the column is constant; `task-lint.sh` reads the
AC table positionally and six columns would break dispatch; asking the agent to reproduce six
per-AC baseline observations costs six compiles of a broken tree and multiplies the
"declared failure text differs from observed" false-`NEEDS_REPLAN` path; D13 already mandates
the oracle — automation was the gap. What the agent gains from seeing polarity (early
detection of an AC that already passes) is delivered instead at dispatch, before any run.

**Tracer-first order — protocol block (R2a) vs unenforceable (R2b).** Verdict: R2b. No
channel enforces order: `verify.sh` sees the final tree, `PreToolUse` cannot judge "edits not
needed by AC-001", and the Haiku evaluator judging prose about order is an unmeasured variance
source. On greenfield Rust with shared test support (`tests/support/mod.rs` compiles against
`App`/`Msg`/`Effect`/`keys`/`ui`), "smallest path to AC-001 green" ≡ stubbing every layer —
the horizontal work the rule forbids — so the rule is unsatisfiable exactly where research 04
wants it most. Adopted instead: author rule (AC-001 = Goal path; baseline = AC-001 command),
lint C9 WARN, metric `changes_before_first_ac1_pass` (transcript replay, zero agent exposure),
and Exp 12 narrowed to a single variable (checklist order only, identical text).

**Per-AC proof leaves — drop (C2, R2a) vs keep as REOPENED targets (R2b, D5).** Verdict: drop,
with one addition. The corpus shows two duplicate kinds: leaf≡leaf (true duplicates) and
parent≡proof-leaf, where the 3.x leaf was the only place a whole-file command ran because step
7 marked parents on children alone — the template's own parents 2.1/2.2 carried evidence the
protocol never executed. Fix: each AC is cited on the item whose evidence is its command, and
a parent with its own `evidence:` is checked only after running it (D27). `REOPENED` targets
that item. Leaves after the drop: template 8, packages 10–12 — all ≥ 5.

**Lint C3 (uncited `R-*`) — ERROR vs WARN.** The skeptic argued WARN because every package
leaves policy requirements (no new deps, final design) uncited and those are enforced by the
whitelist/patterns, not by a leaf. Chosen: ERROR. Traceability is an author-side property
(SpecBench: the executor will not find the gap), the packages were being rewritten anyway,
and the diff-review leaf is the natural home for policy requirements. Cost to authors: one ID
per requirement.

## 7. Changes applied, ranked by expected `/goal` predictability impact

History of the v3.1 branch (`verify.sh`, `gate-selfcheck.sh`, `run-headed.sh`, `VERIFY_BASE_REF`, D24–D32 numbering); v4 equivalents and D31–D39 numbering are in the mapping table at the top.

Mandatory unless marked. "Before/after" are hypothetical walkthroughs, not observed runs.

1. **Two-phase gate run (D24; AGENTS step 8a/8b).** Failure mode: protocol-induced
   circularity — the agent either sets DONE first (and at the turn cap leaves `STATE: DONE`
   with a failing gate, polluting `false_done`), runs `PROGRESS_FILE=` and shows a `DONE` with
   the progress check silently skipped (fools the transcript evaluator), loops, or reports
   NEEDS_REPLAN. Before: "run verify.sh until DONE, then set DONE" cannot terminate. After:
   8a `PROGRESS_FILE= /task/verify.sh` until DONE; 8b set DONE, mark leaves, run the full gate;
   only 8b's output is evidence.
2. **`INCOMPLETE` status (D25).** Failure mode: at "stop after 40 turns" the report grammar
   allows only DONE/BLOCKED/NEEDS_REPLAN, so a truncated run must lie. After: `STATE:
   IN_PROGRESS` stays, `STATUS: INCOMPLETE`, handoff filled — a parseable, honest terminal.
3. **Scope check fail-closed + pinned baseline (D26).** Failure mode: an agent (or its "make
   git stop showing this file" fix) hides a change via rename, `info/exclude`, `skip-worktree`,
   or moves the `baseline` tag; the host gate reports `scope PASS`. After: `--no-renames`,
   `--exclude-per-directory=.gitignore`, hidden-index-entry FAIL, `bash -e` for commands,
   `baseline_sha` recorded at dispatch and applied via `VERIFY_BASE_REF`. Selftest shows six
   scenarios that passed under HEAD now fail.
4. **`gate-selfcheck.sh` (D29).** Failure mode: an AC that already passes at baseline (three
   found in the corpus) or a reference that does not satisfy the gate goes undetected until a
   run wastes 40 turns. After: nop / per-command polarity / oracle at dispatch, `SELFCHECK
   RESULT PASS` required next to `task-lint.sh`. Conditional on the fixture toolchain (runs in
   the container image).
5. **Section 3 dropped, evidence-bearing parents executed (D27).** Failure mode: 24 leaves
   re-run commands with no new evidence (progress % inflates in one turn); parents' evidence
   never executed. After: one item per AC; `REOPENED` has a precise target; template −33 B.
6. **Binding vs hints (D28).** Failure mode: after compaction the agent remembers "hints are
   non-normative" and lets code win over `decisions.md`. After: hints never reference
   `/task/*` (lint C8); `decisions.md` binding in the files table.
7. **Lint C1–C9 (D30)** — author-side, zero agent tokens.
8. **Protocol polish (D31)**: behavior-scope prohibition (in-whitelist drive-by fixes go to
   `FOLLOW_UP`); git-derived `CHANGED:`; flaky check → NEEDS_REPLAN; BLOCKED = environment
   only; resume keyed on `## Log`; `NEXT:` dropped; "what was tried" on every stop;
   post-compaction re-read rule; condition "after the last file change".
9. **Codex harness honesty (D32)** — doc/dispatch correctness; no template effect.
10. **Package defect fixes F1–F5, E7** — the corpus can now dispatch without a guaranteed gate
    failure (F2) or onto the wrong baseline (F3).
11. **Author checklist rules** (optional for the agent, mandatory for authors): real
    refusals; import-never-invent decisions; one `decisions.md` per fixture; finished tasks
    immutable; AC evidence from planner-shipped tests; AC-001 = Goal path; every `R-*` cited.

## 8. Rules moved into tooling instead of prose

History of the v3.1 branch; each script maps to a `taskfmt` subcommand or `verify.toml` field per the table at the top.

- `task-lint.sh`: C1 evidence needs a command or "exits 0" (gate leaf exempt); C2 duplicate
  `P/R/AC/D` IDs; C3 every `R-*` cited by an AC row or checklist item (leaf or ancestor); C4
  placeholder set derived from the template itself, `<...>` inside code spans literal, config
  regex `<[^<>"]+>`; C5 AC coverage on leaves or evidence-bearing parents, identical full
  evidence text on two items → ERROR; C6 WARN AC command absent from `verify.config`; C7 WARN
  `cargo test` multi-filter without ` -- `; C8 ERROR `/task/` in the hints list; C9 WARN
  baseline command ≠ any AC command.
- `verify.sh`: `--no-renames`; `--exclude-per-directory=.gitignore` + untracked `.gitignore`
  listing; hidden index entries (`S`, `h`) → scope FAIL; `bash -eo pipefail -c`;
  `VERIFY_BASE_REF` wins over config.
- `gate-selfcheck.sh`: nop, polarity, oracle; `.patch` or tree reference; `--keep`.
- `selftest.sh`: 9 scope scenarios, `false; true`, two `VERIFY_BASE_REF` cases, 13 lint
  scenarios, 10 selfcheck scenarios.
- `run-headed.sh`/`status.sh`: `baseline_sha`, `baseline_tag_ok`, per-agent prompt block,
  `goal_verdicts: null` for Codex.

## 9. Deliberately unchanged, removed, simplified

Unchanged: D1–D23 architecture; `schema: task/v3` string (v3.1 is the decision set D24–D32 in this note's numbering,
= D31–D39 on main; not a frontmatter bump — no lint or package references a new string); checklist grammar and
5–20 leaf bounds; six prohibitions became seven (the behavior-scope rule has a gate-visible
symptom: `FOLLOW_UP` vs diff); `/goal` condition length (≈ 700 chars of 4,000); commit policy
(allowed, not required — per-leaf commits are a backlog experiment); D17 ordering override.

Removed: README precondition-BLOCKED sentence and checklist-grammar sentence (AGENTS owns
them); README hint "already decided, do not reopen"; per-AC proof section; Handoff `NEXT:`;
duplicate Codex `config.toml` heredoc; manual "prove the gate" README step.

Simplified: AC-003 example to exit-0 form; `CHANGED:` from recall to git output; resume
condition from a state that was always true to one that is true only on resume.

## 10. Remaining weaknesses formatting cannot fix

- Transcript `DONE` forgery: `echo DONE` after a failed gate ends the Claude `/goal` loop
  early. The host gate is immune; `status.sh` ranks it above the transcript; the metric
  `false_done` counts it. Only a hook or harness loop (backlog 6/8) removes it.
- Agent-authored tests (in `src/` `#[cfg(test)]`) can be tautological; nothing proves
  sensitivity until the per-AC revert check (backlog) exists.
- Verticality of a slice needs judgment; lint can only check citations and duplicates.
- Compaction recovery relies on the agent obeying "re-read before editing"; hook injection is
  documented but the issue trail says recovery is hit-or-miss.
- Grok Build: one sentence of vendor text; no CLI, limits, or transcript verified.
- Turn cap in headed mode is model-judged; `--max-turns` exists only in `-p`.
- No run has been executed. Every "predictability" statement above is a hypothesis for §7.

## 11. Verification evidence

Verification evidence — task/v4 port (2026-08-29, working tree before commit).

`cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean. `cargo test`:
146 passed, 0 failed across 9 suites (lib 86; `gate_tamper_matrix` 19 incl. 13 new
scope/command cases; `lint_corpus` 16 incl. 9 negative + positive twins; `selfcheck` 10;
others 5/2/8).

Non-vacuity: the 13 new gate cases were run against the pre-port code first — 6 failed
(staged rename out→in, `.git/info/exclude`, untracked self-ignoring `.gitignore`,
skip-worktree, assume-unchanged, `false; true` errexit); after the port 0 failed. Reverting
`-eo` → `-o` in `run_shell` makes `taskfmt selftest` print
`FAIL cmds: 'false; true' fails focused.1 (errexit)`.

`taskfmt selftest`: `SELFTEST PASS` (adds 3 command-semantics cases and 7 lint mutants
C1/C2/C3/C4/C5×2/C8; tamper matrix renumbered for gate section 3).

`taskfmt lint`: `harness/testdata/example` `errors=0 warnings=1` (C6 on AC-003 `! grep`,
superset via forbidden patterns); TASK-001..007 all `LINT PASS errors=0`, warnings
4/1/1/2/2/2/3 (C6 `taskfmt verify` and regression target-order mismatch in 004–007; C9
baseline≠AC-001 in 001; size 10,072 B in 007). Before the package rewrite the new linter
reported errors 2/3/3/3/3/3/3 (hints `/task/decisions.md`, uncited R-006/R-007/R-003).
`reference/task-template` fails only on placeholders (errors=3), as intended.

Sizes: `reference/task-template/README.md` 5,040 → 5,057 B; `AGENTS.md` 5,153 → 6,354 B;
goal condition block 786 chars (< 4,000).

Diff: 31 files changed, +2,523/−328, plus 4 new files (`selfcheck.rs`, `cmds/selfcheck.rs`,
`tests/selfcheck.rs`, this note); two cherry-picked docs commits `c7a97fb`, `cda275f`.

```text
cargo fmt --check                              clean
cargo clippy --all-targets -- -D warnings      clean
cargo test                                     146 passed, 0 failed, 9 suites
taskfmt selftest                               SELFTEST PASS
taskfmt lint harness/testdata/example          errors=0 warnings=1
taskfmt lint TASK-001..007                     LINT PASS errors=0, warnings 4/1/1/2/2/2/3
taskfmt lint reference/task-template           errors=3 (placeholders only)
```

Not verified (boundary): `taskfmt selfcheck` against a real TASK-00x workspace (needs the run
image toolchain); Codex `/goal` via herdr prompt; any live `/goal` run; the `zai-flash`
profile.
