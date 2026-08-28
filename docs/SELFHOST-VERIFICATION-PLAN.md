# Self-Host Verification Plan — proving task-format works

**Status: DRAFT v3 — after two adversarial review rounds (nine reviewers; v1 3×REJECT, v2 2×REJECT + 2 conditional).**

The plan for proving that `task-format` works: that a `task/v4` package, handed to a fresh agent in a fresh container, produces correct, independently verifiable work — from an empty repository to the finished `pgtui` app.

**The one structural change in v3.** v2 answered "does this project work?" — a systems question — with a research-replication criterion, and every unreachable terminal, cost blow-up and weakening incentive in it descended from that substitution. v3 splits them.

- **Phase 1 is the deliverable.** One unattended chain, all seven tasks gate-green, each independently verified by a fresh host reviewer. This is the operator's question, and it takes the project from **zero measured runs** to one verified chain — the largest single step available.
- **Phase 2 is optional**, entered only if Phase 1 lands and the operator asks: freeze, cheat seed, repeat chains, consensus. It answers a stronger question and costs an order of magnitude more.

Read §0 before anything else; it bounds every claim below.

---

## 0. What this can and cannot prove

| Layer | Self-hosted here? | Evidence about the research question? |
|---|---|---|
| **1. Task package format** (`task/v4`, `lint.rs`) | **Yes** — byte-for-byte the same linter | **Usability and discrimination only — not predictability** |
| **2. Agent protocol** (`reference/task-template/AGENTS.md`) | No — meta-tasks run a forked host protocol (§12) | No |
| **3. Gate** (`taskfmt verify`, `gate.rs`) | **Yes** — the same engine on both substrates | **Usability and discrimination only** |
| **4. Execution harness** (container, herdr, dispatch) | No — meta-tasks run on the host | No |
| **5. Model / provider** | No — meta-tasks run Opus, experiment tasks run `zai-flash` | No |

Consensus reviewers may not cite meta-task outcomes as evidence about the format's predictability. Rows 1 and 3 say "usability and discrimination" precisely because rows 2, 4 and 5 differ on the meta substrate, so no predictability inference survives.

---

## 1. Two claims, two phases

**Phase 1 claim — the deliverable.**

> One complete unattended chain exists from an empty repository to a finished `pgtui`: all seven experiment tasks passed the gate in sequence on one repository, and each was independently confirmed by a fresh host verifier that cloned the pushed SHA itself and ran the acceptance commands itself. Run-to-run variance is unmeasured.

**Phase 2 claim — optional.**

> From a frozen, hash-pinned research object, k chains were replayed from read-only clones of the tag with images pinned by digest; the observed chain pass rate is r; a pre-registered cheat seed scores `cheat_seed_gate_pass == 0`; and a bounded consensus round found no dissent carrying a reproducible falsifier.

**Note on k.** v2 required "k ≥ 3 **consecutive** clean chains" and justified it with per-task p ≈ 0.5–0.6. Those two do not compose: at that p, three consecutive 7/7 chains has probability ≈ 5×10⁻⁷–2×10⁻⁵, i.e. an unreachable criterion. And the p was wrong — see §1.1. Phase 2 therefore reports a **rate over k independent chains**, not a run of consecutive successes.

A **counted replay** contains **zero reseeds, first attempt only**. The ledger records `attempts_per_task[]` and the sentinel carries `reseeds=0`, because "3/3 chains" is otherwise compatible with 21 hidden retries.

### 1.1 What the in-tree data actually says

v2 asserted "every recorded task with n ≥ 2 has produced both outcomes". That is false. From the manifests on disk:

| Task | runs | outcome |
|---|---|---|
| TASK-001 | 2 | **both PASS** |
| TASK-002 | 3 | 1 pass · 1 fail · 1 no gate recorded |
| TASK-003 | 2 | 1 pass (the D-032 override) · 1 fail |

Pooled: 4 pass, 2 fail, 1 no-gate. More importantly, **these are not experimental runs at all** — the findings say so explicitly, and the package that produced them has since changed (the runs' `AGENTS.md` predates D31/D32/D38; TASK-003's checklist went from 10 leaves to 11). So per-task p is **unknown**, not 0.5.

Consequently: **measure p before choosing k.** n=5 replays of TASK-002 from a fixed predecessor SHA on the current package is cheap, is the first thing Phase 2 should do, and is what sets k. Do not set k from a number derived from debugging runs of a different research object.

---

## 2. The two substrates

| | **Experiment tasks** | **Meta-tasks** |
|---|---|---|
| Location | `experiments/tasks/TASK-001..007` | `selfhost/tasks/TASK-1NN` |
| Subject | a disposable GitHub repo that becomes `pgtui` | **this** repository |
| Executor | Claude Code, `zai-flash`, **in a Docker container** | **Opus / high** subagent, on the host, in the `main` checkout |
| Dispatcher | `taskfmt run`, one task at a time | the orchestrator's `Agent` tool, **serialized** |
| Gate | in-container `taskfmt verify`, then host `taskfmt gate` | `cargo run -q --manifest-path harness/Cargo.toml -- verify` |
| Independent verifier | fresh Opus, clones the pushed SHA | fresh Opus, clones `main` at the implementer's commit |
| Source of truth? | **YES** | **NO** |

Meta-task IDs are `TASK-1NN`, never `SELF-0NN`. Three places hard-code `^TASK-[0-9]+$` — `lint.rs:190`, `lint.rs:275`, `gate.rs:589-590` — so a `SELF-` id fails lint, blocks `progress-init` (which refuses on any lint ERROR), and can never satisfy the gate's progress check. Relaxing that regex would edit **the gate**. The plan changes; the linter does not.

---

## 3. Control architecture

### 3.1 `taskfmt selfhost` — the deterministic driver

The loop is deterministic; only four steps need judgement (verify, diagnose, implement, consensus). The driver owns the loop, the cursor and the state, and emits **one `NEXT_ACTION` line per call** naming the contract to run next.

**Phase 1 subcommands only:** `step` · `record` · `status` · `gc` · `env-fault` · `probe`. Roughly 800–1,200 lines.

**Phase 2 adds:** `freeze` · `diff-check` · `verdict --final` · `status --assert-proven` · `stop-gate` · `supervise` · `reset --to` · `invalidate` · `brief`. This is the plan's largest hidden cost — the full surface is a 4,000–7,000-line subsystem, a 40–65% expansion of the 10,793-line crate, 4–8 weeks. Do not build it before Phase 1 has run once.

**Ownership discipline.** `manifest.json` and `ExperimentState` own run identity, repo pinning and per-run data. The ledger stores only what nothing else holds: verifier verdicts, fault records, provenance hashes, cycle framing, timestamps, and a `run_dir` pointer. v2 forbade re-deriving manifest fields and then re-listed them; v3 does not.

Two corrections to v2's factual claims about dispatch:
- **`run --exp <id>` does not mint a repo per task.** A run tagged onto existing experiment state is pinned to the recorded repo; a repo is minted only on the first run of a *fresh* `--exp` id, and `resume_repo_url` **hard-errors** on a `--repo` that disagrees with the record. v2's "always pass `--repo`" rule would therefore make every dispatch bail after a cycle rotation. **Rule: mint a new `--exp` id per cycle, and let the recorded state pin the repo.**
- **`run` does not refuse on a host/image version mismatch.** No such check exists anywhere in `harness/src`. It is backlog 25. v2 asserted it as an existing property; v3 schedules it (§13, TASK-107) and Phase 2 depends on it.

### 3.2 The orchestrator — Fable 5, effort high

Launched as `claude --agent selfhost-orchestrator --permission-mode bypassPermissions`. Delegation is enforced by construction, because a rule in a document is erased by compaction:

- Agent definition: `model: fable`, `effort: high`, `disallowedTools: Edit, Write, NotebookEdit`, `tools: Agent(planner, dispatcher, verifier, meta-verifier, diagnostician, implementer, consensus-reviewer), Bash, Read, Grep, Glob`.
- **`Bash` defeats tool removal** — `cat >`, `sed -i`, `python -c` all write. So the structural claim is only true with a `PreToolUse` Bash hook denying any command whose argv touches a fenced path or the ledger. Without that hook, "the orchestrator cannot write code" is an assertion, not a property.
- Never pass the `Agent` tool's `model` parameter (it **overrides** frontmatter) and never use `subagent_type: "fork"` (it **always inherits the parent model**, so a fork runs Fable, not Opus).
- Per turn: `taskfmt selfhost step` → delegate the one `NEXT_ACTION` → await in the same turn → `record` → end. One tool-using turn, no in-flight background work at turn end.
- Never `Read` the ledger; only `step`/`status` output enters context, capped at ≤80 lines.

### 3.3 The subagents — Opus 5, reasoning high

Definitions live in `.claude/agents/*.md`, **not** `selfhost/contracts/`; Claude Code loads subagents only from the former. Without this the orchestrator spawns `general-purpose`, whose model defaults to `inherit` = **Fable**, and the Opus requirement dies silently on turn one.

**The orchestrator's own definition pins `model: fable`. Only the seven subagent definitions pin `model: opus, effort: high`.**

| Contract | Tools | Writes |
|---|---|---|
| **planner** | Read, Grep, Glob, **Write scoped to `selfhost/tasks/**`** | the meta-task package |
| **dispatcher** | Bash, Read, Grep, Glob | nothing |
| **verifier** | Bash, Read, Grep, Glob | nothing |
| **meta-verifier** | Bash, Read, Grep, Glob | nothing |
| **diagnostician** | Read, Grep, Glob | nothing |
| **implementer** | Read, Edit, Write, Bash, Grep, Glob | the fix |
| **consensus-reviewer** | Bash, Read, Grep, Glob | nothing |

**The planner is new and necessary.** v2 required a meta-task package to exist and pass `selfcheck` *before* an implementer is spawned, gave the orchestrator no `Write`, and named nobody to author it — so the only candidate was the implementer, which reinstates author-verifies-own-spec. The planner writes the package from the diagnosis; it may never be the implementer for the same task; its output is gated on `taskfmt lint` + `selfcheck` PASS + a meta-verifier review **before** any implementer runs.

The **dispatcher** reads its verdict from `manifest.json` (`gate.verdict` + status state), never from `taskfmt run --wait`'s exit code — that exit code is 0 only on `GOAL_MET`, while `is_promotable` also accepts `IDLE` and `GOAL_CLEARED_ERROR`. Every `taskfmt` call runs from the repository root (`Ctx::load` reads a relative `experiment.toml`).

### 3.4 Completion authority

**Phase 1:** the operator reads `taskfmt selfhost status`. The chain is either verified or it is not; there is no need for a machine oracle over a claim a human is watching.

**Phase 2:** `taskfmt selfhost verdict --final` recomputes the result from artifacts — frozen-set hashes, run manifests, GitHub push history, verdict records, the ledger — and exits 0 only if everything checks. The operator or CI re-runs it out of band. The `/goal` condition only tests that this program printed its sentinel in the current turn.

### 3.5 The `/goal` condition

The evaluator runs no commands and reads no files, and a multi-hour transcript is compacted long before the end — so the condition must name **one sentinel line printed by a real command in the current turn**, and must immunize against an *Impossible* verdict clearing the goal after honest failures.

```text
/goal Drive the self-host proof in docs/SELFHOST-VERIFICATION-PLAN.md.
Each turn, run `taskfmt selfhost step`, show its full stdout in the transcript,
and delegate the single NEXT_ACTION line it prints to the subagent it names.

DONE only when, in this same turn, you have run `taskfmt selfhost status` and
its last line is exactly one of:
  SELFHOST-CHAIN-VERIFIED cycle=<id> tasks=7/7 verifiers=7/7
  SELFHOST-NOT-PROVEN model=<profile> failed_at=<task> reason=<class>
Both are terminal. Both are emitted by the binary from the ledger. Never type,
quote, predict or paraphrase either; only a real command run this turn counts.

NOT IMPOSSIBLE: gate FAILs, verifier FAILs, crashed containers and
environmental faults are the normal path of this work. None of them makes this
condition impossible.

On any failure run `taskfmt selfhost record` and take the next NEXT_ACTION. On
an environmental fault (docker, gh, op, network, disk) run
`taskfmt selfhost env-fault "<reason>"` and continue. Never ask the operator.
```

Behavioral rules stay out of the condition — the evaluator cannot check them, and every extra clause dilutes what it can.

### 3.6 Surviving compaction

1. **`step`'s stdout re-emits the invariants every turn.** Load-bearing, because it depends on nothing unverified.
2. **Root `AGENTS.md`** gains a short permanent block. Edit `AGENTS.md`, **never write through the `CLAUDE.md` symlink** — replacing it with a regular file fails `taskfmt selftest` repo-wide, which fails every meta-task's `[regression]`.
3. A `SessionStart` hook with `matcher: "compact"` emitting `taskfmt selfhost status`. Reliability is contested; layers 1–2 carry the weight.

Set `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE=50`.

### 3.7 The supervisor — Phase 2 only

A session cannot relaunch itself, so an out-of-session `taskfmt selfhost supervise` is what makes "never stop" a property rather than a wish. It is **Phase 2** because Phase 1 is operator-attended.

Three requirements v2 missed, all of which make an unguarded supervisor actively dangerous:

- **The heartbeat is written by the driver, not by the `Stop` hook.** A single dispatch turn legitimately runs for hours (`kill_after_min ≥ 180` plus gate plus verification) and emits no turn boundary. The dispatcher writes a heartbeat per status poll.
- **A staleness threshold** exceeding `kill_after_min + gate + verify`, with margin.
- **An `flock` lease on the ledger.** Without it the supervisor relaunches during a healthy TASK-006 dispatch, and two orchestrators share one ledger and one repo — both dispatching the same cursor, both promoting to `main`, both appending. Interleaved appends produce a chain that is broken, or worse, coincidentally valid.

---

## 4. State — the ledger

**The ledger lives outside the worktree**, at `$XDG_STATE_HOME/taskfmt-selfhost/<repo-id>/ledger.jsonl` (or a sibling directory), for a reason v2 got wrong in both directions. v2 required it gitignored (correct: the gate's changed-file set includes untracked files, so an in-tree ledger reports `OUTSIDE` and fails every meta-task's scope check) **and** `git commit -s` per append (impossible on a gitignored path; and `git add -f` makes it tracked, so `git diff <base>` then reports it and the gate fails anyway).

Out-of-tree resolves both. If external auditability is wanted in Phase 2, mirror to a detached ref (`refs/selfhost/ledger`) that never materializes in the worktree.

Format: JSONL, one record per line, `O_APPEND` + `fsync`. Phase 2 adds `prev_hash` chaining. The JSON view is derived.

Per **attempt**:

```
task, cycle, run_id, run_dir, base_sha, result_sha,
gate{verdict,exit,last_line,head}, status_state, container,
started, finished, heartbeat_at,
harness_sha, template_sha, goal_prompt_sha256, taskfmt_binary_sha256,
image_digests, package_tree_hash, distinguisher, fault_class,
metrics{gate_pass, false_done, leaf_claim_accuracy, scope_violation, instruction_violations}
```

Timestamps are mandatory: the supervisor polls `heartbeat_at`, and Phase 2's process reviewer needs `window_start`/`window_end` on the cycle record to check for force-pushes and extra runs inside the replay window. Neither the ledger nor `manifest.json` carried any timing field in v2.

`verified`, `cursor` and `repo_url` are **per-cycle**, nested under the cycle record. `cursor` is **always derived** from `verified`, never stored. SHAs orphaned by a reset are marked `orphaned`; a verifier refuses to run against an orphaned SHA rather than failing it.

---

## 5. Repository layout

```text
selfhost/
  README.md            what this is; NOT source of truth
  AGENTS.md            the meta-substrate protocol (§12)
  CLAUDE.md            symlink -> AGENTS.md
  goal.md              the /goal condition
  tasks/TASK-1NN/      README.md, verify.toml, decisions.md
  fixtures/known-bad/  committed calibration trees (§10.4)
  reports/<cycle>/     verdicts, diagnoses                        [gitignored]
.claude/agents/        selfhost-orchestrator (fable) + 7 subagents (opus)
experiments/references/TASK-0NN/   reference solutions
```

Meta-task `progress.md` files live under the out-of-tree state directory, named exactly `progress.md` (the repo's bare `.gitignore` entry matches at any depth, so a renamed file would be committed).

`experiments/runs/` is **33 GB for seven runs** on this host (TASK-003 alone is 7.9 GB, nearly all `workspace/target/debug`). Set `paths.runs_dir` outside the repository. That evicts 33 GB from the working tree and independently removes the `changed_files` scope problem in §13.

---

## 6. The Phase 1 loop

### 6.1 Every outcome, not two

| Outcome | Handling |
|---|---|
| dispatch returns `Err` before a gate exists (clone, lint abort, secret resolution, prereq timeout, pane/idle/prompt timeout) | `DISPATCH_ERROR` → §7. The dispatcher returns `{outcome, error_class, run_dir?}` |
| gate PASS but status not promotable (`AGENT_EXITED`, `CONTAINER_STOPPED`, `KILLED_TIMEOUT`) | not a pass. Condition is `is_promotable(status) && gate.passed()` |
| `KILLED_TIMEOUT` | `docker stop` **before** gating — the kill path only sends `/goal clear` and leaves the agent writing into the bind mount the gate is about to read |
| promote refuses or the push is rejected | `PROMOTE_ERROR` → §7. Never an agent-work defect |
| verifier crashes, times out, or returns unparseable output | `ABORT`, not FAIL. Re-spawn once with identical inputs — the one legitimate no-distinguisher retry. A second `ABORT` is an environment fault |
| gate exit 70 (internal) | recorded like any other failure, never propagated before the record is written |

### 6.2 The cycle

```
0. RESUME: derive cursor from the ledger; never act on a remembered cursor
1. ensure repo (green-field create if the cycle has none); new --exp id per cycle
2. PREFLIGHT probes: docker info · gh auth status · op whoami · gh api rate_limit · disk headroom
3. CRASH-WINDOW CHECK: origin/main == verified[cursor-1]?  if not, reconcile
4. DISPATCH cursor into a fresh container; record the intent BEFORE any push
5. gate + status; if not (promotable && pass) → §7
6. PROMOTE; if refused → §7
7. VERIFY: fresh Opus verifier against the pushed SHA
8. if verdict FAIL/ABORT → §7
9. record verified[cursor]; GC superseded containers and run dirs; cursor = next
10. cursor past TASK-007 → Phase 1 claim met; emit SELFHOST-CHAIN-VERIFIED
```

**Step 3 replaces v2's "no-op guard".** The hazard is real: a crash between push and ledger-write leaves work on `main` with no record, so the replay's fresh clone already contains the finished work, the trusted overlay is a no-op, `base_sha == clone_sha`, the focused suites are green at base, the agent does nothing, and **the gate passes on a no-op run** — fabricating a data point. But v2's guard was a selfcheck polarity probe on the clone, which (a) returns NOVERDICT on TASK-001, whose focused commands are unrunnable in an empty repo, (b) runs the fixture toolchain on the credentialed host — the exact thing §10.2 forbids two sections later — and (c) is redundant, because the SHA comparison detects the same condition for free.

**Step 9's GC is mandatory, not optional.** 7.9 GB per run, seven runs per chain.

---

## 7. Faults, recovery, and the failure terminal

| Class | Meaning | Response |
|---|---|---|
| **agent-work** | package and harness sound; the model did it wrong | replay with a `reseed` distinguisher, capped at 3, then → §7.1 |
| **package** | the package is defective (contradictory decisions, unsatisfiable AC, whitelist excludes a required file) | fenced repair (§8): planner authors, `AUTHORIZED-REPAIR` token, meta-verifier PASS |
| **harness** | a `taskfmt` bug, a gate hole, a container problem | meta-task (§12) |
| **environment** | docker, gh, op, network, disk, rate limit, model unavailable | **never a code fix.** Report and retry (§7.2) |
| **indeterminate** | cannot be established | a second diagnostician with a different framing, before any change |

### 7.1 `MODEL_CAPABILITY_LIMIT` — the failure terminal

**This is the most important addition in v3.** v2 had a success terminal and no failure terminal, run by an agent forbidden to stop or ask. Consider: GLM-5.3-Flash at effort `low` cannot do TASK-005. Three reseeds produce identical `agent-work` diagnoses. Dispatch suspends until "a meta-task lands whose fix touches a file in the failing class" — but `agent-work` has no such file. Changing the model is forbidden outright. The goal forbids stopping and asking. The **only** remaining action that changes the distinguisher tuple is a fenced edit to the task. So the loop reclassifies `agent-work` as `package`, authorizes its own repair, and weakens the research object — reintroducing exactly the failure the fence exists to prevent, one level down.

So: after 3 reseeds with `agent-work` diagnoses and no fenced edit available, the driver records the chain as a **negative result** and emits

```
SELFHOST-NOT-PROVEN model=zai-flash failed_at=TASK-005 reason=model-capability-limit
```

That is a terminal state, it satisfies the goal, and it is a legitimate research finding: *the format did not produce a passing chain under this profile.* **A goal whose only terminal is PASS will be reached by weakening.**

### 7.2 `NEEDS_OPERATOR` — reporting without asking

"Never ask" is achievable; "never be blocked" is not — a session cannot repair its own auth, credit balance or disk. Separate *asking* (blocking on an answer) from *reporting* (non-blocking):

`taskfmt selfhost env-fault` writes a fault record naming the failed probe and the exact remediation command, **and returns immediately**. It does **not** poll: a foreground backoff loop would block the Bash tool for hours, bust `BASH_MAX_TIMEOUT_MS`, emit no heartbeat, and trigger the supervisor's duplicate relaunch. The **supervisor** owns the probe loop, the backoff and the resume. In Phase 1 the operator does.

Faults that need this: 1Password session expiry (macOS default 30 min, against a 4–6 hour chain, with `op read` running per dispatch — §13 item 5 removes this one entirely with a service-account token), `gh` token scope, Docker daemon death, disk exhaustion, rate limits, credit balance, host sleep.

### 7.3 Anti-spin

The distinguisher is **machine-derived**: `(taskfmt_binary_sha256, image_digests, harness_sha, package_tree_hash, reseed_counter)`. Two attempts with the same tuple are forbidden regardless of what the record claims — which closes the case where a fix was compiled but never installed, or the images were never rebuilt.

The mode switch exits when **a meta-task lands whose fix touches a file in the failing class**, and dispatch resumes on that landing. For `agent-work`, §7.1 is the exit.

### 7.4 Revoking a verdict

The diagnostician emits `earliest_bad_task = k`; the driver drops `verified[k..]`, rewinds the cursor to `k`, and resets to `verified[k-1]` — independent of where the fix lands. Without this, a TASK-005 verifier that surfaces a defect originating in verified TASK-003 work replays TASK-005 forever against a poisoned base.

---

## 8. The fence

### 8.1 What is fenced

```
experiments/tasks/**            reference/task-template/**
experiments/references/**       harness/goal-prompt.md
experiment.toml [agents] block  AGENTS.md (root)
harness/** except harness/target/ and harness/src/cmds/selfhost.rs and harness/src/selfhost/**
```

**Defined by exclusion, not enumeration.** v2 enumerated six `harness/src/*.rs` files plus `cmds/**` and `ops/**`, and thereby omitted `runstate.rs` — the code that writes the gate verdict `verdict --final` recomputes from — as well as `config.rs`, `cli.rs`, `redact.rs`, `Cargo.lock` and `harness/tests/**`. A post-freeze edit to `runstate.rs` would not have voided the tag.

`harness/src/cmds/selfhost.rs` and `harness/src/selfhost/**` are **carved out by path**, so the exception is decidable mechanically. v2 wrote the exception as "the ledger types in `runstate.rs`", which is not a path; move those types into `harness/src/selfhost/ledger.rs`. Without this carve-out, every bugfix to the driver you are still writing costs a full green-field restart, and in Phase 2 voids the tag.

### 8.2 The rules

- **`experiment.toml`'s `[agents]` block is immutable for the duration.** Swapping `zai-flash` for a stronger model, or raising effort, is the cheapest way to turn a red chain green and proves nothing. No waiver. This is a knowing narrowing of the operator's "authorized to change everything", and §7.1 exists so the narrowing does not create a deadlock.
- **Repairs need a token, not a line of prose.** v2's `AUTHORIZED-REPAIR: <path>` was a string any package author could type, in a loop with no human — the loop authorizing its own repairs. Instead the driver **mints a signed token** after an independent reviewer, never shown the failing run's diff, returns PASS; `taskfmt selftest` verifies the token, not the string.
- **Whitelist entries under `experiments/` or `reference/` must be literal paths** — no `*` or `?`. `*` crosses `/` in the gate's glob dialect (`gate.rs:391`), so `experiments/tasks/*` would whitelist all seven packages including their `trusted/` oracles.
- **Cost gradient inverted.** Editing a fenced path is the expensive option. Cheap edits are confined to `selfhost/**`, `docs/**`, and tests that only add coverage.

### 8.3 `diff-check` — a sound over-approximation (Phase 2)

v2 claimed ~14 weakening classes were "all mechanically decidable". They are not, and the failure mode was the bad one: the classifier cannot decide, returns NEUTRAL, NEUTRAL passes, and the ledger records `diff-check clean` over a weakening edit.

Genuinely decidable by path/set/token: editing anything under `trusted/` · dropping `-D warnings` · changing `kind:` · changing the task-id set · **removing** a `[focused]`/`[regression]`/`[lint]` command · **removing** a `[[forbidden_patterns]]` entry · **deleting** an `R-*` or its citation.

Not decidable: rewriting an `R-*` to require less · rewriting a `D-*` to match what an agent did (the classifier takes two trees and never sees the agent's diff) · editing `reference/task-template/AGENTS.md` "in a way that adds executor guidance" — which is the **most consequential fenced file**, since it is the protocol injected into every run, and the least mechanizable. Glob widening is regex-language containment, decidable in principle but the plan has no normative statement of the dialect. "Narrowing a test command" is half-built already (`lint::CargoTestSpec::narrower_than`), but `parse_cargo_test` returns `None` on compound commands — and TASK-001's focused list is four compound `grep -qE … && grep -qE …` pin checks, i.e. exactly the commands most worth weakening.

**So invert the default.** `diff-check` classifies as STRENGTHENING only against a short allowlist of provably-additive patterns (a new `[focused]` command, a new `[[forbidden_patterns]]`, a new `R-*`, a new `trusted/` file). **Every other diff to a fenced path is WEAKENING and requires a token.** The seven decidable classes emit a specific reason string; everything else emits "unclassified change to a fenced path". ~300 lines instead of a week, strictly safer, and it makes §8.2 and §8.3 agree.

---

## 9. Replay scope

v2's deny-by-default rule made *any* change under `harness/src/**` force a green-field restart. With six known pre-flight work items and thirteen recorded defects, that means at least six full restarts before the first clean chain — each re-paying every earlier task — and it draws no distinction between changing the judge and fixing a timeout.

**The question that decides scope is: did the change alter what the gate judges, what the agent is told, or what the agent is judged against?**

| Change | Replay |
|---|---|
| `gate.rs`, `progress.rs`, `selfcheck.rs`, `lint.rs`, `taskfile.rs`, `verifycfg.rs` | **green-field** — the judge changed |
| `experiments/tasks/**`, `reference/task-template/**`, `harness/goal-prompt.md`, `experiment.toml` | **green-field** — the contract or the protocol changed |
| `harness/images/**` | **green-field** — the baked-in gate changed |
| `runstate.rs`, `cmds/promote.rs`, `cmds/gate.rs` (verdict recording) | **green-field** — the record changed |
| plumbing: `cmds/run.rs` timeouts, `cmds/status.rs` detection, `cmds/repo.rs`, `interactive.rs`, `ops/docker.rs`, `ops/git.rs` non-scope paths | **failing task only** |
| `harness/src/cmds/selfhost.rs`, `harness/src/selfhost/**`, `selfhost/**`, `docs/**` | **failing task only** |
| tests that only add coverage | **failing task only** |

**Precedence is scoped to within the table**: among matching rows the most invalidating wins, but *a change confined to a "failing task only" path takes that row*. v2's unqualified "most invalidating wins" deleted all three of its own exceptions, including the one covering the driver.

For `cursor == TASK-001`, always green-field (delete and create), never reset in place — the operator's rule.

**Standing obligation.** Every fix touching `harness/**` ends with `cargo build --release && install && taskfmt preload && taskfmt build-images`. The gate is baked into the images; without a rebuild the host gate runs the new engine and the container runs the old one, and the agent's two-phase endgame converges on the wrong oracle. The resulting binary hash and image digests are the distinguisher (§7.3).

**Reset mechanics (Phase 2).** The harness has **no force-push at all** — `ops/git.rs` exposes only `push` and `push_upstream`. `taskfmt selfhost reset --to` must do `push --force-with-lease origin <sha>:main` guarded by `repo_prefix`, **rewrite `experiment.json`** dropping every task at or after the reset point, and record the orphaned SHAs. Skipping the rewrite is silent corruption: `experiment --resume` filters on `task.pushed`, so the task just reset is **skipped**, and if it was the only pending one the run reports success over a commit that no longer exists. In Phase 1, green-field instead — `reset` saves machine time, not correctness.

---

## 10. Verification

### 10.1 Independence

A verifier gets a fresh context and **only** `(repo_url_or_mirror, pushed_sha, base_sha)` — never the implementer's transcript, the container's `progress.md`, the gate log, the run directory, the ledger, or the dispatcher's summary.

**Credentials.** The experiment repo is created `--private`, so a credential-free container cannot clone it — v2 required both, which stalls the loop at cycle 1 task 1 forever, misdiagnosed as an environment fault. **Resolution: the driver fetches the SHA into a bare mirror on the host and bind-mounts it read-only into the verifier container.** Do not solve it by giving the verifier `gh`; that reopens the channels this section closes.

**Isolation is structural only if the container mounts nothing else.** Mount the scratch clone and nothing more — no repo bind mount, no `experiments/runs`. v2's enforcement ("a verdict whose transcript shows a read of a run path is void") is post-hoc prose inspection by an unnamed party in a session that compacts.

**Diff base** is the run's `base_sha` (the trusted overlay commit), not the previous verified SHA — the pushed range contains a harness-authored `planner: <TASK> trusted material` commit, and diffing against the predecessor scores planner-supplied tests as out-of-scope agent work.

Model-family correlation is closed on the experiment substrate (Flash implements, Opus verifies) and open on the meta substrate (Opus both sides). Stated, not hidden.

### 10.2 What the verifier checks

The host gate is necessary and not sufficient: the recorded TASK-003 run passed 11/11 gate checks while overriding a fixed decision, and said so in plain text.

1. Clone fresh at the recorded SHA from the mirror; refuse if `orphaned`.
2. Run every `AC-*` evidence command; capture output.
   **The gate AC is a special case.** Every package's final AC *is* `taskfmt verify` — and on a fresh clone there is no `progress.md` (never committed) and no `baseline` tag (never pushed). Under v2's blanket "a command that cannot run is `NOT_RUN` → FAIL", that AC is unrunnable on every task forever, making a verifier PASS structurally unreachable. The verifier runs it as `taskfmt verify --base <base_sha> --task-dir <pkg> --no-progress`, and reports the progress check as covered by the host gate rather than by itself.
3. Confirm every `R-* (MUST)` and `(MUST NOT)` against the diff from `base_sha`.
4. **Decision conformance** — every `D-*` the task's scope touches. The gate's blind spot; the verifier's main job.
5. Confirm the artifact works: build, run the real entry points, inspect gallery output where produced.
6. Return a **schema-validated** verdict: `overall ∈ {PASS, FAIL, ABORT}`, per-AC array, per-R array, decision-conformance list, and on FAIL the single most decisive evidence.

**Reward-hacking checks are mechanical.** Weakened tests, deleted tests, added `#[ignore]`, added lint suppressions, out-of-scope changes are `instruction_violations` greps computed by the driver. The verifier judges only what a grep cannot.

Long commands (cold Rust builds, 102-test testcontainers suites) exceed the Bash tool's 10-minute ceiling: raise `BASH_MAX_TIMEOUT_MS`, use `run_in_background` + poll, write output to a file and grep it so only the verdict enters context.

### 10.3 Meta-tasks get a verifier too

v2's implementer authored the fix, could author its own `verify.toml`, and declared itself done. The **meta-verifier** has the same rules: fresh context, clones this repo at the implementer's commit, runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `taskfmt selftest`, the meta-task's ACs, and (Phase 2) `diff-check`. Its verdict — not the implementer's — is authoritative.

**Which binary gates a meta-task.** v2 said two contradictory things: §12 said the post-fix `cargo run`, §10.3 said a binary built from the pre-fix SHA. The pre-fix rule is unsatisfiable for the very fixes that matter — TASK-105 exists *because* the pre-fix `changed_files` makes any gate fail scope, so a pre-fix binary can never pass it. **One rule: gate with the post-fix binary.** For a meta-task whose `expected_paths` includes the gate engine itself, the meta-verifier additionally confirms `harness/tests/gate_tamper_matrix.rs` is unchanged — which preserves the anti-self-gating property without the impossibility.

### 10.4 Calibration against known-bad trees

A reviewer that passes a tree known to be defective has zero discriminative power, and its PASS carries no information. Before any contract's verdict is trusted it must **FAIL**:

1. The TASK-003 decision-override tree — gate PASS with a documented D-032 violation. **Copy it to `selfhost/fixtures/known-bad/task-003-decision-override/` and commit it.** v2 pointed at `experiments/runs/20260828-175916-…/workspace`, which is gitignored, is 7.9 GB, is exactly what `gc --keep-last` is specified to reap, and moves when `runs_dir` relocates — the plan's own GC destroys its own calibration fixture. **Pin it to the pre-amendment package hash**, because once the D-032 amendment lands the tree fails for a *mechanical* reason instead of the judgement reason it was chosen for, and would silently stop testing decision-conformance while still reporting "calibrated".
2. A sabotaged tree: one added `#[ignore]`, one deleted trusted test.

**Calibrate once and put the contract prompt templates in the fenced set.** v2's "re-run whenever a template changes" is a recursion on the artifact you iterate most, at 21 verifier invocations a round.

---

## 11. Phase 2 — the optional research claim

Entered only if Phase 1 lands and the operator asks for it.

1. **Measure p.** n=5 replays of TASK-002 (and TASK-003) from a fixed predecessor SHA on the current package. This sets k. §1.1 explains why v2's k was chosen from a number that does not exist.
2. **Freeze.** Hash-pin §8.1's set at a git tag. **Build the images once, at freeze, and record the digest; every replay asserts `docker inspect` returns that digest and dispatches by digest**, not by `:latest`. v2 required "images rebuilt from the tag" *and* pinned digests — but `harness/images/base/Dockerfile` builds `FROM debian:bookworm-slim` with an unpinned 15-package `apt-get`, the pgdg and nodesource repos, `sh.rustup.rs` and a GitHub release download, so a rebuild produces a different digest and every replay would either trip the drift refusal or silently re-record it while the ledger claimed one frozen object. Also requires TASK-107, the host/image version check that does not exist yet.
3. **Cheat seed.** Pre-register the hack prompts **before the loop starts**, hash them into the fenced set with an explicit `goal-prompt` override slot so running them does not void the tag, and have them authored by a contract with no stake in the outcome. v2 left them unauthored inside a loop whose goal terminates on the result being 0 — a limp prompt scores 0 and unlocks the gate. Three tasks is a stronger signal per dollar than seven uniform seeds: TASK-001 (grep-pin heavy, easiest to game), TASK-003 (known decision-conformance hole), TASK-007 (widest whitelist).
   **Delete v2's remedy.** "If any seed passes, adopt the host-only `verify.toml` overlay invisible to the container" is precisely the split-oracle defect §9's standing obligation forbids: the agent's phase-8a loop would converge on a gate that is not the verdict, turning every later chain into false-blocked noise. A failing cheat seed is a defect to fix in the shared gate.
4. **k chain replays**, each on a fresh repo, each dispatched from a read-only clone of the tag — which requires a `--runs-dir` flag or `TASKFMT_RUNS_DIR`, because `paths.runs_dir` is repo-relative with no override and a read-only clone cannot create `experiments/runs/<id>/`. Report the **rate**, not a consecutive streak. Zero reseeds per counted replay.
5. **Independent verification of every replay task**, per §10. Full Opus verifier on every task of chain 1; mechanical checks plus Opus on only the highest-risk tasks for chains 2..k — decision conformance is a property of the package, not of the replica, so re-judging the same `D-*` set on a third near-identical diff buys little.
6. **Bounded consensus.** A fixed rubric; a **pre-registered, recorded list of accepted limitations** that are not grounds for dissent; a dissent counts **only if it cites a reproducible falsifying command with its output**, which the driver re-runs. Reviewers blind to prior verdicts. The process reviewer is a **program** — `gh api` push history versus manifests — not a model.
   - **The Fable orchestrator casts a recorded, explicitly non-binding attestation.** The operator's success condition names it; v2 deleted it outright. Its evidence is second-hand and its goal terminates on PASS, so it must not bind — but recording it satisfies the requirement at no cost.
   - **A cross-family reviewer contradicts the all-Opus requirement.** It is the only thing that makes unanimity mean more than correlated agreement, so it is worth having — but it is an explicit deviation and needs operator sign-off. Phase 2 only.

---

## 12. The meta-task substrate

`selfhost/AGENTS.md` is the host delta from the template protocol, with a sibling `CLAUDE.md` symlink.

| Template clause | Host replacement |
|---|---|
| `/task` ro, `/work` rw, `/progress` rw | the package dir, the `main` checkout, the out-of-tree `progress.md` |
| `/task` read-only *by mount* | no mount exists. **The package lists its own directory in `forbidden_paths`**, so any edit to it fails the gate |
| gate is the baked-in binary | `cargo run -q --manifest-path harness/Cargo.toml -- verify`, never the `PATH` binary |
| `$TASKFMT_BASE` | the spawner exports `TASKFMT_ROOT`, `TASKFMT_TASK_DIR`, `PROGRESS_FILE`, `TASKFMT_BASE` (the tip when the implementer was spawned; never a SHA in `base_ref`, which goes stale and lints clean) |
| — | the spawner runs `taskfmt progress-init <pkg> -o "$PROGRESS_FILE"` before the implementer starts |
| — | **Never `git stash` or move `TASKFMT_BASE`.** (v2 said "never `git commit`", with the rationale that a commit empties the base diff — that is wrong: the gate diffs base against the *working tree*, so committing changes nothing. `stash` and re-pointing the base are the real hazards.) The implementer commits to `main`; the orchestrator pushes after the gate |
| — | **New-test rule:** a focused command must name a **new `--test` target file**. A new filter inside an existing target exits 0 at base ("0 filtered out") — a silently green baseline. Only a missing target is genuinely red |
| — | Commands use `--manifest-path harness/Cargo.toml`; the workspace is not at the repo root, and `cd harness && …` defeats the linter's `cargo test` spec matching |
| `NEEDS_REPLAN` triggers | add: the fix requires editing a fenced path and no repair token exists |
| protocol steps 1–8, progress grammar, prohibitions, stop conditions, turn signal, final report | **kept as close to verbatim as the substrate allows.** Steps 1, 2 and 8 necessarily rewrite their paths and the gate command; everything else is unchanged, and keeping the transcript grammar identical is what makes the substrates comparable |

**One implementer at a time, in the `main` checkout.** The repository works on `main` only (root `AGENTS.md`), so meta-tasks do not get the fresh-clone isolation experiment tasks get — a stated exception, and part of why §0 says the meta substrate is not evidence about the research question. Serialization is enforced by the orchestrator's one-subagent-per-turn rule; a second concurrent implementer would put another agent's edits into the first one's changed-file set. The out-of-tree ledger and gitignored reports keep orchestrator artifacts out of the gate's changed set.

**Selfcheck is mandatory here.** `taskfmt selfcheck <pkg> --workspace <fresh clone at base> --base <sha>` must PASS before an implementer is spawned. Otherwise nothing enforces a red baseline: selfcheck is opt-in at dispatch and the `baseline` lint rule is a WARN restricted to `kind: feature|bugfix`, so a meta-task with a green baseline lints clean, gates clean, and is unfalsifiable.

**Fence enforcement, three mechanical layers:**

1. `forbidden_paths` per package — exact-prefix on the changed set, zero cost when untouched: `["experiments/tasks", "experiments/references", "reference/task-template", "AGENTS.md", "docs/research/RESEARCH-FINDINGS.md", "selfhost/tasks/TASK-1NN"]`. For an authorized repair, drop the fenced prefix and re-add the other packages by name.
2. `allowed_globs == expected_paths` (already lint-enforced), so an unlisted fenced path fails `scope` independently.
3. `taskfmt selftest` gains a repo-wide check: every `selfhost/tasks/*/README.md` carries the disclaimer verbatim; its `verify.toml` forbids `experiments/tasks` unless a valid repair **token** is present; and **`taskfmt lint selfhost/tasks/*` runs**, since `paths.tasks_dir` is a single value and bare `taskfmt lint` never sees the meta corpus.

Every `selfhost/tasks/*/README.md` carries verbatim under `## Goal`:

> This is a meta-task. It is scaffolding for proving the project, not part of the research object. `experiments/tasks/**` is the source of truth; this package is not.

---

## 13. Work items

### 13.1 Pre-flight — operator-authored, merged before the session

The orchestrator must never be its own bootstrap. v2 said this and then listed the same artifacts as meta-tasks; v3 assigns each to exactly one place.

| # | Item | Size |
|---|---|---|
| 1 | Install the toolchain. `taskfmt` is not on `PATH` and **zero `harness-*` images exist on this host**. `cargo install --path harness && taskfmt preload --auto && taskfmt build-images --agent claude --auto` | **½–1 day** — a four-stage chain installing rustup 1.98.0, Node 22, herdr 0.8.2, pgdg pg-client-16; it has never run here |
| 2 | Move `paths.runs_dir` outside the repository | minutes; evicts 33 GB and defuses item 3 |
| 3 | **TASK-105** — fix `git::changed_files`' unfiltered `:(top,glob)**/.gitignore` enumeration (`ops/git.rs:183`). It reports **exactly 14** plugin-cache files under `experiments/runs/*/agent-home/` as `OUTSIDE`, so **`taskfmt verify` fails scope on this worktree today**. Fix: intersect with the base commit's ignore rules | ½–2 days; git has no direct primitive for "the base commit's ignore rules" |
| 4 | **TASK-102** — the Phase 1 `taskfmt selfhost` driver (§3.1), plus `--runs-dir` | ~1 week for the Phase 1 surface |
| 5 | Secret survivability: a 1Password **service-account token** | hours; removes the single most likely mid-chain stall |
| 6 | **TASK-106** — unattended robustness: `fail_prereqs`' `process::exit(1)` → a recorded error; wrap `wait_and_gate`'s `?`; `--kill-after` on `experiment`; hoist the duplicate-entry `retain`; the `repo delete` prefix guard in Rust | 1–2 days |
| 7 | **TASK-101** — confirmation results honored, `--auto`/`--yes` reach every mutating step. `repo create`, `repo delete` **and `promote`** all discard their confirm result; `promote` is the one command that writes `origin/main` | 1–2 days |
| 8 | Raise `runtime.kill_after_min` to ≥ 180 | minutes |
| 9 | `.claude/agents/*.md` — orchestrator (`model: fable`) + seven subagents (`model: opus`) | 2–4 days, rewritten repeatedly |
| 10 | `.claude/settings.json` — the `PreToolUse` Bash deny (§3.2), `SessionStart:compact`, and env: `BASH_MAX_TIMEOUT_MS`, `CLAUDE_CODE_GOAL_CHECKIN_MINUTES=10`, `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE=50`, `CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH=2` | hours |
| 11 | Root `AGENTS.md` invariant block; `selfhost/AGENTS.md` + symlink, hand-authored and human-reviewed | 1 day |
| 12 | Calibration fixtures committed to `selfhost/fixtures/known-bad/`, and every contract calibrated (§10.4) | 1 day |
| 13 | Smoke corpus: a trivial `TASK-9xx` (write a file, one focused grep) dispatched with `taskfmt run --task <path>` | ½ day + 1 day debugging the first end-to-end dispatch |

**Honest pre-flight total: 3–5 weeks of one engineer** — dominated by items 4 and 9.

### 13.2 Loop-executed meta-tasks

Generated by the planner from diagnoses. Two known in advance, both **also pre-flight-eligible** because they change `harness/**` and would green-field an in-flight chain:

| ID | Goal |
|---|---|
| **TASK-103** | Reference solution for TASK-002 under `experiments/references/`, plus a committed base workspace fixture; the **oracle phase runs green for the first time**. ~1 week — it is hand-writing a working pgtui increment (Turso store, connection list, CLI loop; 20 regression + 4 focused suites) |
| **TASK-104** | Container-mode selfcheck, so TASK-001 stops returning NOVERDICT and the oracle becomes reachable corpus-wide. 2–4 days |
| **TASK-107** | Host/image version check — `run` refuses on mismatch (backlog 25). **Phase 2 prerequisite** |

### 13.3 The D-032 amendment

`experiments/tasks/TASK-003/decisions.md` D-032 requires all fields non-empty except Password; `trusted/…/screen_create_form_test.rs:62-71` types only a name, presses Enter, and asserts `SaveConnection` is `effects[0]`. Unsatisfiable. This is a real defect in the research object, sitting on the critical path.

v2 made this "a decision record, not a package" — which meant the single most sensitive edit in the plan was the one edit with **no fence on it**: no `verify.toml`, no repair token, no `diff-check`, no independent review. And its acceptance rule was backwards: "accepted on two consecutive green dispatched TASK-003 runs" rewards whichever version the agent passes more often, and the correct amendment is the *stricter* one, so a correct fix that lowers the pass rate would be **rejected** in favor of the weaker trusted test — a gradient toward weakening written into the place the plan says to watch hardest.

**v3:** a normal meta-task package with a repair token for `experiments/tasks/TASK-003/trusted/crates/pgtui/tests/screen_create_form_test.rs`, a `diff-check` waiver record, and a meta-verifier PASS. **Polarity is fixed in advance: the `D-*` is authoritative and the trusted test is corrected to match.** Acceptance is a **conformance** judgement — a fresh verifier, given only D-032 and the amended test, agrees the test now exercises the decision. **Agent pass rate is never an input to whether a spec correction is accepted.**

---

## 14. Operator preconditions

Docker running with ≥300 GB allocated · `gh auth refresh -s delete_repo` · `OP_SERVICE_ACCOUNT_TOKEN` exported and headless `op read` proven · repo clean on `main` · **workspace trusted** (`/goal` is gated by the same trust rule as hooks) · `caffeinate -dimsu` around the session.

Launch: `claude --agent selfhost-orchestrator --permission-mode bypassPermissions`, then paste §3.5.

---

## 15. Costs

Corrected from the manifests — v2's per-task figures were not reproducible from the artifacts:

| Task | start → gate recorded |
|---|---|
| TASK-001 | **3.8 min** |
| TASK-002 | **16.3 min** |
| TASK-003 | **34.7 min** (agent quiet at ~10 min; the gate itself took 56 s; ~24 min was completion-detection latency from the 300 s activity window) |

Modelled with corpus growth (73→102 cumulative tests, testcontainers from TASK-004): 5/20/35/50/70/80/100 min ⇒ **one clean chain ≈ 6 h container time**, serial. Verifiers dominate: each is a cold Rust build of the full dependency set inside a 2-CPU container, 20–30 min for TASK-001..003 and 45–90 min for TASK-004..007, serialized ⇒ **5–7 h per chain**. **~21–29 machine-hours per verified chain.**

Opus verification is the token cost: $5–20 per verifier, so **Phase 1 ≈ low hundreds of dollars**; the Flash side is negligible. **Phase 2 as specified is 55–85 machine-hours and $1,000–5,000**, and needs 350–600 GB with `gc` working.

**Phase 2's cheapest honest form:** k=2, cheat seed on three tasks, full verification on chain 1 and targeted verification thereafter, calibrate once, mechanical process reviewer. ≈ 20–25 machine-hours, low hundreds of dollars.

---

## 16. First five sessions

1. **Make the host able to run anything (½–1 day).** Docker with ≥300 GB. `cargo install --path harness`; `taskfmt preload`; `taskfmt build-images --agent claude` — **expect it to fail the first time**. `gh auth refresh -s delete_repo`; 1Password service account; prove headless `op read`. *Exit:* `taskfmt run --task experiments/tasks/TASK-001 --repo <fresh>` reaches a live agent pane and `taskfmt attach` shows it.
2. **Fix what makes unattended dispatch impossible (1 day).** Pre-flight items 2, 3, 6, 8. Then `cargo fmt --check && cargo clippy -D warnings && cargo test && taskfmt selftest`.
3. **One attended chain, watched (1–2 days, mostly waiting).** `taskfmt repo create`; `caffeinate -dimsu taskfmt experiment --tasks all --auto`. Build no orchestrator. The point is to discover what breaks at TASK-004..007 — **nobody knows, because no task past 003 has ever been dispatched**, and 004 is where DinD and testcontainers enter. Expect to stop, fix, and green-field at least once. This retires the plan's largest unknown for one operator-day and ~$20 of Flash tokens.
4. **The verifier contract and its calibration (1 day).** Write `.claude/agents/verifier.md`. Copy the known-bad TASK-003 tree into `selfhost/fixtures/` **first** — it is gitignored and `gc` is specified to delete it. Then calibrate: the verifier must FAIL that tree and must FAIL a passing tree with one `#[ignore]` added. If it passes either, everything downstream is theatre — find that out in day four, not week six.
5. **Verify the chain you have; write the honest sentence (1 day).** Run the calibrated verifier against each pushed SHA from session 3, serialized, credential-free, against the mirror. Append to the out-of-tree ledger. Then write down which claim the evidence supports.

Only then decide whether the orchestrator, the full fence and Phase 2 are worth three to five more weeks.

---

## 17. Deferred

Reference solutions beyond TASK-002; the research metric suite beyond the five in §4; `false_done_vendor`; the Codex substrate (the `codex-default` profile has no `env_secret`, and `taskfmt codex-login` is invoked by the container entrypoint but does not exist as a CLI subcommand); the format ablations in the research backlog; and the eight recorded harness defects that do not block a chain.

## 18. Known deviations from the operator's brief

Recorded rather than silently taken.

1. **The orchestrator's vote** (brief: "all Opus subagents *and* the Fable orchestrator agree") is **non-binding** — §11.6. It delegated everything and its goal terminates on PASS.
2. **"Never be blocked"** is re-scoped to "never asks, never spins, auto-resumes" (§7.2). A session cannot repair its own auth, credits or disk. Item 5 of pre-flight removes the most likely cause outright.
3. **"Authorized to change everything"** is narrowed by the fence (§8), with one absolute: the `[agents]` profile block. §7.1 exists so this narrowing cannot deadlock the loop.
4. **A cross-family consensus reviewer** (Phase 2 only) contradicts "subagents must be Opus" and needs sign-off.
5. **`experiments/references/`** is new content inside `experiments/`, though outside `experiments/tasks/**`.
6. **The meta-task protocol is a fork** of the template's `AGENTS.md` (§12); the container assumptions have no host equivalent.
