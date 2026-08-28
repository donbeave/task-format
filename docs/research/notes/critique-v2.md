# Critique of the v2 task template (adversarial review)

Sources reviewed: `docs/research/raw/01-initial-goal-template.md` (doc01) and
`docs/research/raw/02-checklist-checkboxes.md` (doc02). Line numbers below refer
to those files. "v2" = the `goal-task/v2` reference file at doc02 L251-737.

Project goal restated: the structure that gives the MOST PREDICTABLE output from
one fresh agent (Claude Code `/goal` or Codex) in an isolated container, for one
bounded task. Predictability = low variance across runs + deterministic outer
gate can decide pass/fail. Every argument below is judged against that.

Thesis: v2 optimizes for *completeness of prose*, not for predictability. It puts
the burden of state-keeping on the LLM (five hand-maintained projections of one
state), asks the agent to mutate the contract it is told is immutable, and cannot
be enforced as written in a read-only container. Most of its rules are neither
needed by the agent to act nor checked by any gate; they are attention cost.

---

## 1. Length and attention cost

### Measured size

| Artifact | Lines | Words | Bytes | Est. tokens |
| --- | --- | --- | --- | --- |
| v2 `task.md` template (doc02 L251-737) | 487 | 4,017 | 27.8 KB | ~7,000-7,500 |
| of which author-validation HTML comment (L689-736), removed at instantiation | 48 | 560 | 3.8 KB | ~1,000 |
| `progress.md` template (L201-234) | 34 | 244 | 1.6 KB | ~400 |
| `/goal` invocation prompt (L161-187) | 27 | ~180 | | ~300 |
| **Instantiated package the agent reads before any code** | | | | **~6,500-7,500** |

An instantiated task does not get "considerably shorter" (doc01 L71): placeholders
are replaced by real content (code-flow explanation, 3+ AC blocks, evidence
table, 10-20 checklist leaves each with an evidence command). Expect 7-10k tokens
of task package + repository `CLAUDE.md`/`AGENTS.md` before the first file read.
That is not fatal for a 200k window, but it is all *instruction* text, and it is
read once at the start and then progressively diluted by tool output. After a
compaction (doc01 L334 admits this happens) the agent is told to re-read the
whole thing again (L555 step 10).

### Rule density

Counting normative statements (MUST / MUST NOT / "do not" / "only") in the v2
body, not the author comment:

- Header blockquote L287-297: 5 rules
- Checklist semantics L451-468: 13 bullets
- Verification contract L507-517: 9 conjunctive completion conditions
- Task-contract integrity L521-530: 4
- Checklist execution protocol L544-558: 13 numbered steps
- GOAL_PROGRESS rules L568-574: 5
- Progress log rules L578-600: ~5
- Prohibited shortcuts L602-621: 15 bullets
- Blocked/replan L623-642: ~8
- Completion report L644-687: fixed grammar with 7 sections

≈ 75-80 distinct normative rules, not ~40. Empirically, fresh agents follow the
*salient, structural* instructions (the checklist they can see, the verifier they
can run, the report grammar at the very end) and drop the diffuse ones. The rules
most likely dropped are exactly the ones v2 adds over v1: UTC timestamps per
checkpoint, per-turn `GOAL_PROGRESS`, parent roll-up ordering, `REOPENED`
entries, the `VERIFYING` state transition, "record non-obvious choice in the
progress log AND the completion report" (L436). None of those are checked by
`verify.sh`, so their omission is invisible to the gate but their presence adds
variance in what the agent spends turns on.

The predictability argument is simple: every rule the agent must remember but no
machine checks is a source of run-to-run variance with zero enforcement. Cut them
or make them machine-checked.

### Redundancy map (same rule stated in N places)

| Rule | Where it is stated in v2 |
| --- | --- |
| "Do not edit checklist text/IDs/order" | frontmatter `allowed_mutations` L263-267; header L297; semantics L457, L467; forbidden touch points L380; integrity L527-528; prohibited L606-607; `/goal` prompt L185-186 (8 places) |
| "`verify.sh` exit 0 + last line DONE" | frontmatter L274-277; leaf 4.3 L492; canonical gate L509-510; protocol step 12 L557; report L667-670; `/goal` prompt L180 (6 places) |
| "Only leaves count / exactly one current leaf" | frontmatter L271-272; semantics L460-461; GOAL_PROGRESS rules L570-571; prohibited L611-612 (4 places) |
| "Check only after evidence succeeds" | semantics L465; protocol L550-551; progress log L600; prohibited L609; `/goal` prompt L170-172 (5 places) |
| "Do not neutralize verifier / weaken tests" | forbidden touch points L381; prohibited L613-616; `/goal` prompt (doc01 L94-95) (3 places) |
| Completion conditions | doc01 L106-118 (5 conds); v2 L507-517 (9 conds); protocol step 12 L557; `/goal` prompt L177-180 — four lists, none identical |
| Progress log block | doc02 L206-224 (`progress.md` template) and L582-598 (inside `task.md`) — **and they differ**: L208 has `NOT_STARTED`, L584 does not; L216 has `REOPENED`, L592 does not, yet L600 and L231 require reopening. The spec has already diverged from itself, the very failure mode doc02 L25 warns about. |
| Run the checks | leaf 3.x (prove each AC), leaf 4.2 (run all focused and regression checks), leaf 4.3 (`verify.sh`, which per doc01 L292-297 runs the same checks again) — three passes over the same tests |
| Diff review | leaf 4.1 L490; protocol (doc01 step 8 L620, dropped from v2 protocol but the leaf stays); prohibited L619-620 |

Sections that are entirely redundant with a machine check and can be cut:
`Task-contract integrity` (L521-530, harness-side concern, agent cannot act on
it), `checklist:` config in frontmatter (L268-273, config for a linter that does
not exist; L612 even lets a task change `max_active_leaf_items`, which
introduces per-task protocol variance), `Checklist semantics` (L451-468, spec of
the notation, belongs in a shared protocol file, not repeated per task), the
whole `Per-turn progress signal` section (see section 3).

---

## 2. Contradictions and ambiguities inside v2

1. **"Immutable" vs mutable regions.** doc01 L47 and L435 call `task.md` an
   "immutable execution contract"; doc01 L192 states the design reason for
   keeping status out of the file: "Otherwise executor must modify the same
   document that defines its contract." v2 does exactly that (L438-447 status
   block, L474-495 checkbox region) and renames the header to "Protected
   execution contract with a narrowly mutable progress surface" (L287). The
   cost is not cosmetic: whole-file hashing and read-only mounting (doc01
   L308-313, the only *deterministic* protections listed) are both lost, and
   replaced by an unspecified "normalize the two allowed mutable surfaces
   before hashing" (L530). What "values" inside the status block means is
   undefined: is the HTML comment on L443-444 mutable? Is free text allowed in
   the backticks of `Last checkpoint`? A normalizer that accepts arbitrary text
   inside backticks accepts arbitrary prose injected into the contract.

2. **`progress_log` path vs package layout.** Frontmatter L262 says
   `.goal/progress/TASK-000.md`; the package tree L30-33, README L241-244, and
   doc01 L49 say `tasks/TASK-042/progress.md`; doc01 L180 and L319 say
   `.goal/progress/TASK-042.md`. Three documents, two answers. An agent will
   pick one; runs will differ.

3. **`verify.sh` location.** Frontmatter L275/L279 and the invocation (doc01
   L7) use `./verify.sh` at repository root; the package tree L33 puts it at
   `tasks/TASK-042/verify.sh`. `protected_paths` L279 lists `./verify.sh` which
   is then not the file in the package.

4. **`protected_paths` self-contradiction.** doc01 L186 lists the task file in
   `protected_paths`; v2 L280-282 says "Do not list the whole task file here".
   Fine, but then L380 makes "static content of this task contract" a
   forbidden touch point with no machine definition of "static content".

5. **Leaves 1.1-1.3 are not verifiable.** L476: 1.1 is "complete when the
   relevant constraints and code flow can be stated accurately in the
   transcript" — self-assessed, invisible to any gate, and meaningless
   headless. L477: 1.2 "supported by a command, repository state, or named
   artifact" — the evidence for a precondition is whatever the agent says it
   is. L478: 1.3 "captured in the progress log" — checkable only as "a log
   line exists". These three leaves inflate `verified_leaves` by 3 (20-30% of a
   10-15 leaf task) with zero evidentiary value, which makes the progress
   percentage a worse signal, not a better one.

6. **4.3 / 4.4 / 4.5 are circular or self-referential.** L492 leaf 4.3 runs
   `verify.sh`. Completion condition 4 (L512, "every checklist leaf and parent
   is checked consistently") and 5 (L513, state `DONE`, current `NONE`) are
   listed under the canonical gate. If `verify.sh` implements conditions 4-5,
   it can never pass at 4.3 because 4.3, 4.4, 4.5 and parent 4 are still
   unchecked when it runs — permanent loop. If `verify.sh` does not implement
   them (the sensible reading), then nothing the agent runs checks them, and
   4.4 ("reconcile checklist hierarchy and evidence through item 4.3", L493)
   is a leaf whose evidence is a log entry about log entries. 4.5 (L494) says
   "prepare the required completion report in the progress log" while L646
   says the report goes "in the final response". Two destinations. After 4.3's
   `verify.sh` run the agent still performs at least five more mutations
   (check 4.4, 4.5, parent 4, set `State: DONE`, set current `NONE`, write
   log, print GOAL_PROGRESS). The tree `verify.sh` blessed is not the final
   tree. Only an outer re-run can bless the final tree, so the leaf 4.3 in the
   agent's checklist is at best advisory.

7. **Transient invalid state is mandated.** L444: current is `NONE` "only when
   NOT_STARTED or DONE"; L461: exactly one unchecked leaf is current. Between
   checking the last leaf and setting `State: DONE` there is no unchecked leaf
   and the state is not `DONE`. If the container dies there (or the turn ends
   and the gate snapshots), the snapshot is invalid by the spec's own rules.

8. **`VERIFYING` boundary undefined.** L556 step 11: set `VERIFYING` "when
   implementation leaves are complete". Are the `3.x` proof leaves
   implementation? The checklist has no marker separating implementation from
   verification leaves, so different runs flip the state at different points.
   The state carries no information the checkbox set does not already carry.

9. **Template violates its own checklist-validation rule.** Rule 22 (L718):
   "no artificial single-child hierarchy merely to reach four levels". The
   template's own example (L481-482) has `2.1.1` with exactly one child
   `2.1.1.1`, admitted at L472 as being there to "demonstrate the required
   `4.2.7.5`-style address format". Generated tasks will copy the example.

10. **Five hand-maintained projections of one state.** Checkbox tokens
    (L474-495), `Leaf progress` counter (L445), `progress.md`
    `LEAF_PROGRESS`/`CURRENT_ITEM` (L585-586), the `GOAL_PROGRESS` line
    (L565), and the report `CHECKLIST` block (L655-660). doc02 L25 rejects a
    second checklist because "duplicate task lists would eventually diverge",
    then duplicates the *counters* four times. Counters are exactly the thing
    LLMs get wrong (off-by-one, stale after reopen). Anything derivable
    (`n/total`, percent, `next`) must be derived by the harness, never written
    by the agent.

11. **Parent roll-up judgement.** L458 and L106: parent checked only when
    every descendant is checked *and* "the parent's stated outcome is true"
    *and* "any parent-level verification condition succeeds". A parent whose
    descendants are all checked but whose roll-up is "false" is a planner
    error, not an executor state; giving the executor discretion here
    reintroduces the subjective judgement the design was trying to remove.

12. **Depth-first numeric order vs real dependency order.** L466: "Do not skip
    an eligible earlier leaf merely because a later item is easier" but "unless
    this task explicitly states a different dependency order". There is no
    field for that order. Tasks that need `2.2` before `2.1.2` (common: remove
    the legacy path first so the new tests are meaningful) must encode order
    by renumbering, which fights the "IDs are stable" rule.

---

## 3. Failure modes in a container

Setting: `docker run` fresh container, repo checked out, task package mounted,
agent launched headless (`claude -p "..."` / `codex exec`). Outer harness
collects the result.

1. **`GOAL_PROGRESS` has no consumer.** doc02 L132 and doc01 L104 justify the
   line by the `/goal` evaluator judging from transcript evidence. Headless
   `claude -p` gives one non-interactive run; the interactive `/goal`
   re-evaluation loop that consumes per-turn transcript signals is not what
   runs there. Codex has no such evaluator either. The per-turn line
   (L562-566) then costs one instruction, one output line per turn, and a
   consistency obligation (L554 "must match the task status and checkbox
   state") that nothing verifies. If the harness wants a stdout signal, it
   wants exactly one final parseable line, not one per turn. Section 4 keeps
   only the final report.

2. **Timestamps.** L446, L550, L590, L232 require ISO-8601 UTC per checkpoint.
   In a container `date -u` works, but the agent must spend a tool call per
   checkpoint to obtain it, or it fabricates a plausible time (the common
   outcome; models routinely emit stale or invented timestamps). Fabricated
   timestamps are worse than none: they look like evidence. No gate condition
   uses them. The harness already timestamps every tool call in its own log.
   Cut; if ordering matters use the natural line order of the log.

3. **Read-only mount vs checkbox toggling.** doc01 L308 ("Mount `task.md` and
   `verify.sh` read-only") is the only deterministic protection proposed. v2
   requires writing checkbox tokens and status values into `task.md`. These are
   mutually exclusive. With the file mounted `:ro`, Claude Code `Edit` gets
   `EROFS`; the agent then (observed patterns) retries, tries `chmod`, tries
   `cp`+`mv` (fails on a bind-mounted file), tries `git checkout`, or concludes
   the environment is broken and reports `BLOCKED` per L355 — a false BLOCKED
   caused by the template, not the task. Without the `:ro` mount, protection
   degrades to prose plus an unspecified normalizer, and any agent bulk
   `Write` of the file (reflowed whitespace, CRLF, trailing newline, `- [ ]`
   spacing changed by Codex `apply_patch`) trips the normalizer and yields a
   false tamper verdict. Either way predictability drops.

4. **`tasks/TASK-042/` inside the repo pollutes the scope diff.** If the task
   package lives in the working tree, `git diff` and "final diff contains only
   task-related changes" (L514) must special-case it, and the agent may
   `git add` it. Keep the package outside the repo.

5. **Hash-checking individual fixture files.** `protected_paths` inside the
   repo (L280-282) cannot practically be bind-mounted `:ro` one by one; the
   only workable protection is harness-side hash comparison after exit. That
   is fine, but it means the agent-facing prose about protected paths is a
   courtesy, and the gate must be the authority. Do not pretend otherwise in
   the template.

### Proposed mount layout (resolution)

```text
/task/               read-only bind mount (owned by harness)
  task.md            contract + static checklist definition
  AGENTS.md           execution protocol, shared across all tasks
  verify.sh          code-level gate; run as: cd /work && /task/verify.sh
/work/               read-write; the repository checkout
/work/.goal/         read-write, gitignored; excluded from scope diff
  progress.md        the ONLY file the agent maintains for state
```

Rules that follow from the layout, none needing prose:

- Contract and verifier tamper is impossible, not merely prohibited
  (`EROFS`). Hash checks are still run post-exit for the in-repo
  `protected_paths`.
- Checklist *state* lives in `/work/.goal/progress.md`; checklist *text and
  IDs* live in `/task/task.md`. No normalizer needed: the harness parses
  `progress.md` (a trivial grammar) and compares its ID set to the IDs in
  `task.md`.
- The harness, not the agent, re-runs `/task/verify.sh` on the final tree.
  The agent's own runs are for its feedback loop only.
- Codex parity: identical layout, `AGENTS.md` content goes into
  `AGENTS.md`/instructions; nothing in the protocol references Claude-specific
  slash commands.

---

## 4. Minimal viable v3

Design rule: a section survives only if (a) the agent needs it to *act*, or (b)
the harness *checks* it. Anything else is deleted or moved to the harness.

### `/task/task.md` (read-only, per task, target 1,500-2,500 tokens)

Keep:

- Frontmatter: `id`, `title`, `verify: /task/verify.sh`, `protected_paths`,
  `expected_paths` (globs; used by the harness for the scope metric, and by
  the agent as orientation — one field, two consumers). Drop `schema`,
  `parent`, `depends_on`, `base_ref`, `subsystem`, `progress_log`,
  `task_contract`, `checklist:` — none are used by the agent in-container;
  DAG metadata belongs to the orchestrator.
- Goal (one sentence, a state).
- Context: ordered read list with one-line "why", code-flow paragraph, baseline
  command + expected pre-change output. (Merge v2 `Observable outcome`, `Why
  this task exists`, `Starting point` into one section; three headings for
  what is one narrative.)
- Preconditions `P-*` each with a *command* that proves it. If a precondition
  has no command it is not a precondition, it is context.
- Scope: in / out. Drop "expected touch points" prose (now frontmatter
  `expected_paths`) and "forbidden touch points" (now `protected_paths` +
  `EROFS`).
- Requirements `R-*`.
- Acceptance `AC-*`: **one table**, columns `ID | Given/When/Then | evidence
  command | expected result`. Merge v2's `Acceptance criteria` (L396-416) and
  `Acceptance-to-evidence map` (L532-542); two representations of the same
  rows is a divergence source and ~500 tokens.
- Fixed decisions `D-*`.
- Checklist (static plan): IDs, max depth 2, 5-15 leaves, every leaf =
  `ID — what becomes true (R/AC refs) — evidence: <command> → <expected>`.
  No checkboxes in this file. No leaves for "read context", "confirm
  preconditions", "reconcile checklist", "prepare report" (v2 1.1, 1.2, 4.4,
  4.5): the first two are protocol steps, the last two are harness checks.
  Keep one baseline leaf (v2 1.3) because it has a real command. Keep one
  final leaf "verify.sh passes" because it is the agent's stop signal.

Cut entirely from `task.md`: `Executor discretion`, `Execution status`,
`Checklist semantics`, `Task-contract integrity`, `Checklist execution
protocol`, `Per-turn progress signal`, `Progress and handoff log`, `Prohibited
shortcuts`, `Blocked and replan policy`, `Completion report`, author-validation
comment (move to a task linter). These are not task-specific; repeating them
per task means every task instance re-states the protocol, and any drift
between instances changes agent behavior for reasons unrelated to the task.

### `/task/AGENTS.md` (read-only, shared across all tasks, target 600-900 tokens)

- Read order: `task.md`, repo instructions, listed context, then
  `.goal/progress.md` if it exists (resume).
- Protocol in ≤8 steps: verify preconditions (report `BLOCKED` on failure);
  run baseline; work leaves in ID order, one at a time; before marking a leaf
  `DONE`, run its evidence command; write `progress.md` after each leaf;
  when all leaves done run `verify.sh`; review `git diff` against scope; emit
  final report.
- `progress.md` grammar (below).
- Prohibited list condensed to the six that matter and that the gate can
  detect symptoms of: do not modify verifier/protected paths; do not
  skip/weaken/delete failing checks; do not special-case fixtures; do not
  suppress errors/lints/exit codes; do not change files outside scope; do not
  claim `DONE` without a `verify.sh` run in this session. (v2 L617-618 on
  compat layers is task content, expressed as `R-005`; not protocol.)
- `BLOCKED` vs `NEEDS_REPLAN` in two sentences each, plus "do not spin: stop
  when no evidence-backed action remains".
- Final report grammar (below).

Rationale for the split: the protocol becomes an independent experimental
variable. Same `AGENTS.md`, different `task.md` → measures task-format effect.
Same `task.md`, different `AGENTS.md` → measures protocol effect. v2 entangles
both in one file, so no experiment can separate them.

### `/work/.goal/progress.md` (read-write, agent-owned, machine-parsed)

```text
TASK: TASK-042
STATE: IN_PROGRESS            # IN_PROGRESS | DONE | BLOCKED | NEEDS_REPLAN
BASELINE: cargo test expired_refresh_token -> FAIL (expected)
LEAVES:
1.1 DONE  cargo test expired_refresh_token -> exit 0, 1 passed
1.2 DONE  rg legacy_expiry_check src -> no matches
2.1 TODO
2.2 BLOCKED  fixture refresh-token-contract.json missing
NEXT: 2.1
NOTE: chose helper name validate_expiry (incidental)
```

- One line per leaf ID from `task.md`. State ∈ `TODO | DONE | BLOCKED`.
  Evidence is the command and its observed result, one line. No timestamps, no
  counters, no percent, no parent roll-ups (harness derives all of it).
- Harness checks, all deterministic: ID set in `progress.md` == leaf ID set in
  `task.md`; `STATE: DONE` ⇒ all leaves `DONE`; every `DONE` leaf has
  non-empty evidence; optionally the harness **re-runs each leaf's evidence
  command** from `task.md` and compares with the claimed state (this converts
  "false leaf claims" from unmeasurable to measurable).

Why the checklist state should NOT stay in `task.md`: (1) the `:ro` mount is
the only tamper protection that does not depend on a parser; (2) the
normalizer needed for in-place checkboxes is itself a source of false
positives; (3) a fresh or resumed agent gets the same information from two
short files; (4) the doc02 L25 divergence concern applies to duplicated *text*,
not to a state file that carries only IDs — ID-set equality is a one-line
check. Why not copy the checklist text into `progress.md`: then it is a second
list and does diverge; IDs only.

### Final report (stdout, last thing the agent prints)

Keep v2's shape but drop the derivable/duplicated blocks: `CHECKLIST` (harness
derives from `progress.md`), `CONTRACT_INTEGRITY` (agent cannot know; harness
hashes), `PROTECTED_PATHS`. Keep `STATUS`, `TASK`, `SUMMARY`, `ACCEPTANCE`
(per AC: PASS/FAIL + command), `VERIFY` (exit code, last line), `CHANGED`,
`DEVIATIONS`, `FOLLOW_UP`. Single terminating line `GOAL_RESULT
task=<id> status=<STATUS>` for cheap parsing; this replaces per-turn
`GOAL_PROGRESS`.

### Expected effect on predictability

- Agent-maintained state drops from 5 projections to 1 file with 1 grammar.
- Rules the agent must hold drop from ~75 to ~20, all of which either direct
  an action or have a detectable violation.
- Tamper protection becomes filesystem-enforced; no parser in the trust path.
- Every measurable property (section 5) is computed by the harness from
  `progress.md`, `git diff`, hashes, and a harness-run `verify.sh` — the
  agent's self-report is never load-bearing for pass/fail.

---

## 5. What must be measured per experiment run

Each (template variant × task × seed) run produces one record. Compare
variants on distributions, not single runs; predictability = pass rate AND low
variance.

| Metric | How computed (harness-side, never from agent claims) |
| --- | --- |
| `gate_pass` | harness re-runs `/task/verify.sh` on final tree: exit 0 AND last stdout line `DONE` |
| `false_done` | agent final `STATUS: DONE` AND `gate_pass` false |
| `false_blocked` | agent `BLOCKED`/`NEEDS_REPLAN` on a task whose preconditions the harness knows are satisfiable (seed a control set) |
| `protected_tamper` | count of `protected_paths` whose post-run hash ≠ pre-run hash; plus count of `EROFS`/`chmod`/`mv` attempts on `/task` seen in the tool log |
| `scope_violation` | files in `git diff --name-only` (excluding `.goal/`) not matching `expected_paths`; count and list |
| `leaf_claim_accuracy` | for each leaf marked `DONE`, harness re-runs its evidence command; fraction whose claimed result reproduces |
| `state_consistency` | `progress.md` parses; ID set equality with `task.md`; `STATE: DONE` ⇔ all leaves `DONE`; no `DONE` leaf without evidence |
| `report_conformance` | final report parses against the grammar; `GOAL_RESULT` line present and last |
| `ac_coverage` | ACs whose evidence command the agent actually executed at least once (from tool log) / total ACs |
| `verify_runs` | number of `verify.sh` invocations by the agent; time/turn of first run (early first run predicts convergence) |
| `turns`, `tool_calls`, `input_tokens`, `output_tokens`, `wall_seconds`, `cost` | from harness/CLI JSON output |
| `compactions` | count of context compactions (Claude Code emits them in stream-json) |
| `diff_stability` | across N seeds of the same task: Jaccard similarity of changed-file sets, and pairwise diff size ratio; low spread = predictable |
| `instruction_violations` | tool-log greps for known anti-patterns: edits under `/task`, `--no-verify`, `#[ignore]`/`skip`/`xit(` added, lint-suppression comments added, test files deleted |

Minimum experiment design: ≥3 tasks of different kinds (bugfix, feature,
removal) × ≥5 seeds × each template variant. Report per variant: mean and
std-dev of every numeric metric, and counts for the boolean ones. A variant that
raises `gate_pass` but also raises `false_done` or `scope_violation` variance is
not more predictable.

---

## 6. Alternatives worth prototyping later

1. **No checklist at all** (`task.md` = goal/context/R/AC-with-commands/D +
   `verify.sh`, `progress.md` free-form). Hypothesis: for ≤15-leaf tasks the
   checklist changes `gate_pass` by <5 points and the tokens saved reduce
   `turns`; the checklist earns its place only on resume-after-compaction.
2. **Executable checklist** (each leaf is a script/test in `/task/leaves/`;
   progress = which leaf scripts pass, computed by harness, agent never writes
   state). Hypothesis: `false_done` and `leaf_claim_accuracy` become exact
   (100%) because there is nothing to claim; cost is planner effort.
3. **Harness-driven loop instead of `/goal`** (`claude -p` once; harness runs
   `verify.sh`; on failure re-invokes with `--continue` and the verifier
   output as the prompt; hard budget on iterations). Hypothesis: more
   predictable than the in-agent loop because the stop condition is external
   and identical for Claude Code and Codex.
4. **Protocol-location ablation** (identical task content; protocol inline in
   `task.md` vs in `AGENTS.md` vs in the launch prompt). Hypothesis: no
   difference in violations; validates the v3 split and lets `AGENTS.md` be
   frozen across experiments.
5. **Rule-count ablation** (prohibited list with 6 vs 15 vs 25 items).
   Hypothesis: violation rate of the six core rules is unchanged; extra rules
   only add tokens.
6. **Fresh reviewer pass** (second `claude -p` with `task.md` + final diff +
   verifier output; outputs PASS/FAIL per R/AC). Hypothesis: catches
   `scope_violation` and semantic misses that `verify.sh` cannot express,
   with lower variance than asking the implementer to self-review (v2 leaf
   4.1).
