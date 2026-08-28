# Self-Host Verification Plan — proving task-format works

**Status: DRAFT v2 — revised after five adversarial reviews (verdicts: 3 REJECT, 2 ACCEPT-WITH-CHANGES). Under review.**

**What this document is.** The implementation plan for a self-hosted meta-experiment whose purpose is to prove that the `task-format` project works: that a `task/v4` package, handed to a fresh agent in a fresh container, reliably produces correct, independently verifiable work — from an empty repository to the finished `pgtui` app.

**v1 → v2, in one paragraph.** v1 put the proof in the wrong place. Its terminal authority was a Haiku transcript judge; its meta-tasks were verified by the agents that wrote them; its anti-spin rule contained a written-in livelock; its cost gradient pushed the loop toward weakening the research object; and its evidence was one lucky chain. v2 inverts the control (a Rust driver owns the loop, the model layer only supplies judgement), freezes the research object before the terminal gate, requires k≥3 held-out replays, adds an independent verifier for meta-tasks, and replaces the "never blocked" fiction with a `NEEDS_OPERATOR` state that reports without asking and resumes on its own.

---

## 0. What this can and cannot prove

State this before anything else, because the whole plan is bounded by it.

| Layer | Self-hosted by this plan? | Evidence about the research question? |
|---|---|---|
| **1. Task package format** (`task/v4`, `lint.rs`) | **Yes** — byte-for-byte the same linter and format | **Yes** |
| **2. Agent protocol** (`reference/task-template/AGENTS.md`) | No — meta-tasks run a forked host protocol (§12) | No |
| **3. Gate** (`taskfmt verify`, `gate.rs`) | **Yes** — the same engine on both substrates | **Yes** |
| **4. Execution harness** (container, herdr, dispatch) | No — meta-tasks run on the host | No |
| **5. Model / provider** | No — meta-tasks run Opus, experiment tasks run `zai-flash` | No |

So: the meta run is evidence that the **format and the gate** are usable and discriminating. It is **not** evidence about the protocol, the harness or the model — that evidence comes only from the experiment substrate, and only from the frozen replays in §11. Consensus reviewers may not cite meta-task outcomes as evidence about the format's predictability.

---

## 1. What "proven" means

> The project is **proven** when, from a frozen and hash-pinned research object, **k ≥ 3** consecutive unattended chains — each on a newly created repository, each dispatched from a read-only clone of the frozen tag, each with images rebuilt from that tag — produce a gate PASS on all seven experiment tasks; each of those 21 task outcomes is independently confirmed by a fresh host verifier that cloned the pushed SHA itself and ran the acceptance commands itself; a Harbor-style cheat seed scores `cheat_seed_gate_pass == 0` against the same frozen object; and a bounded consensus round finds no dissent carrying a reproducible falsifier.

Any change to the frozen set resets k to 0 and voids the tag.

**If cost forbids k ≥ 3**, the claim must be weakened rather than the criterion: *"one complete unattended chain from an empty repository to a finished `pgtui` exists and was independently verified; run-to-run variance is unmeasured."* n=1 supports that sentence and nothing stronger. Say which sentence is being claimed.

**Why k ≥ 3 and not 1.** The in-tree data contradicts any assumption of low variance. Every recorded task with n ≥ 2 has produced both outcomes: TASK-002 (1 pass / 1 fail / 1 no-gate), TASK-003 (1 pass — and that pass overrode a fixed decision — / 1 fail). At an implied per-task p ≈ 0.5–0.6, a single 7/7 chain has probability ≈ 0.008–0.03; even at an optimistic p = 0.9 it is ≈ 0.48. One green chain is a coin flip, not a result. Post-freeze replays are also the only genuinely **held-out** sample: everything before the freeze is the loop fitting itself to these seven tasks.

**Additionally**, for the two tasks with observed variance (TASK-002, TASK-003), run n ≥ 5 replays from the verified predecessor SHA and report pass rate ± stddev. `3/3 chains; TASK-002 4/5; TASK-003 5/5` is a defensible claim.

---

## 2. The two substrates

The single most important distinction in this plan.

| | **Experiment tasks** | **Meta-tasks** |
|---|---|---|
| Location | `experiments/tasks/TASK-001..007` | `selfhost/tasks/TASK-1NN` |
| Subject | a disposable GitHub repo that becomes `pgtui` | **this** repository |
| Executor | Claude Code, `zai-flash` profile, **in a Docker container** | **Opus / high** subagent, on the host, in the `main` checkout |
| Dispatcher | `taskfmt run` (one task at a time) | the orchestrator's `Agent` tool, **serialized — one implementer at a time** |
| Gate | in-container `taskfmt verify`, then host `taskfmt gate` | `cargo run -q --manifest-path harness/Cargo.toml -- verify` |
| Independent verifier | fresh Opus, clones the pushed SHA | fresh Opus, clones `main` at the implementer's commit |
| Source of truth? | **YES** | **NO** |

Meta-task IDs are `TASK-1NN`, not `SELF-0NN`. Three independent places hard-code `^TASK-[0-9]+$` — `lint.rs:190`, `lint.rs:275`, and `gate.rs:589-590` — so a `SELF-` id fails lint, blocks `progress-init` (which refuses on any lint ERROR), and can never satisfy the gate's progress check. Relaxing that regex would edit **the gate**, which by §9 costs a full green-field replay, paid to rename scaffolding — and it is exactly the failure mode §16 names. The plan changes; the linter does not.

---

## 3. The frozen set and the fence

### 3.1 The frozen set

`taskfmt selfhost freeze` records the SHA-256 of every path below, plus the built image digests, into the ledger at a git tag:

```
experiments/tasks/**            reference/task-template/**
experiments/references/**       harness/goal-prompt.md
experiment.toml [agents] block  harness/src/{lint,taskfile,verifycfg,gate,progress,selfcheck}.rs
harness/src/cmds/**             harness/src/ops/**            harness/images/**
```

Every dispatch re-verifies the frozen set and refuses on drift without a recorded waiver. The frozen set is what §1's k counts against.

### 3.2 The fence

The v1 fence covered only `experiments/tasks/**`. That left the actual independent variables editable. The fence now covers the whole frozen set, and:

- **`experiment.toml`'s `[agents]` block is immutable for the duration of the goal.** Swapping `zai-flash` for a stronger model or raising effort is the cheapest way to turn a red chain green and proves nothing. Forbidden outright, no waiver.
- **A repair of any fenced path requires an `AUTHORIZED-REPAIR: <path>` line in the meta-task README**, a diagnosis id, and a PASS from a reviewer that was **never shown the failing run's diff**.
- **Whitelist entries under `experiments/` or `reference/` must be literal paths** — no `*` or `?`. The gate's glob dialect makes `*` cross `/` (`gate.rs:391`), so `experiments/tasks/*` would whitelist all seven packages including their `trusted/` oracles. A lint rule enforces literal-only.
- **The cost gradient is inverted.** v1 priced package edits as cheap (replay one task) and harness fixes as expensive (green-field). A loop told to minimize cost therefore preferred editing the research object. Now: **any edit to a fenced path costs a green-field restart plus an independent review of the edit itself.** Cheap edits are confined to `selfhost/**`, `docs/**`, and tests that only add coverage.

### 3.3 `taskfmt selfhost diff-check`

Prose cannot fence anything an agent can rationalize past, so weakening is detected mechanically. `diff-check <frozen> <worktree>` classifies every change to a fenced path as **WEAKENING / NEUTRAL / STRENGTHENING**. WEAKENING refuses without a waiver. All of the following are mechanically decidable and all are WEAKENING:

removing or narrowing a `[focused]`/`[regression]`/`[lint]` command · editing or deleting anything under `trusted/` · widening `allowed_globs`/`expected_paths` · removing a `[[forbidden_patterns]]` entry · dropping `-D warnings` · deleting an `R-*` or its citation · substituting a weaker AC evidence command · moving a requirement from `In scope:` to `Out of scope:` · rewriting a `D-*` to match what an agent did · changing `kind:` to dodge kind-gated lint · editing `reference/task-template/AGENTS.md` in a way that adds executor guidance · changing the task-id set.

---

## 4. Control architecture

v1 made the model layer the driver and a Haiku transcript judge the completion authority. That is the hole this project exists to close, reproduced one level up. v2 inverts it.

### 4.1 `taskfmt selfhost` — the deterministic driver

The loop in §7 is deterministic: dispatch, gate, promote, verify, record, decide replay scope. `taskfmt experiment` already implements two thirds of it. Only four steps need judgement (verify, diagnose, implement, consensus). So the driver owns the loop and the state, and emits **one `NEXT_ACTION` line per call** naming the subagent contract to run next.

Subcommands: `step` · `record` · `brief` · `status [--assert-proven]` · `freeze` · `diff-check` · `reset --to <sha>` · `invalidate --exp <id> --task <id>` · `gc --keep-last <n>` · `env-fault` · `supervise` · `verdict --final`.

Scope discipline: the ledger is a **sidecar keyed by experiment id and run id**. It must not re-derive `repo_url`, `cursor` or per-attempt data that `ExperimentState` (`runstate.rs:143`) and `manifest.json` (`runstate.rs:28-65`) already hold. Genuinely new: the host-verifier verdicts (`verified[]`), the reset/invalidate operations, provenance hashes, and the fault records.

### 4.2 The orchestrator — Fable 5, effort high

Launched as `claude --agent selfhost-orchestrator --permission-mode bypassPermissions`. Delegation is enforced **structurally, not textually**, because a rule in a document is erased by compaction:

- Its agent definition sets `disallowedTools: Edit, Write, NotebookEdit` and `tools: Agent(dispatcher, verifier, meta-verifier, diagnostician, implementer, consensus-reviewer), Bash, Read, Grep, Glob`. **It cannot write code.** That also closes the §3 hazard: an orchestrator with no `Edit` tool cannot quietly loosen a fenced path.
- Its contract forbids passing the `Agent` tool's `model` parameter (which **overrides** frontmatter) and forbids `subagent_type: "fork"` (which **always inherits the parent model** — a fork would run on Fable, not Opus).
- Each turn: run `taskfmt selfhost step`, show its stdout, delegate the single `NEXT_ACTION` to the named subagent, await the result **in the same turn**, `taskfmt selfhost record`, end turn. One tool-using turn, no in-flight background work at turn end.
- It must never `Read` the ledger. Only `brief`/`step`/`status` output enters its context, each capped at ≤80 lines.

### 4.3 The subagents — Opus 5, reasoning high

`selfhost/contracts/*.md` are **not** agent definitions; Claude Code loads subagents from `.claude/agents/*.md`. Put the definitions there (with `selfhost/contracts/*` as relative symlinks, or not at all), each pinning:

```yaml
model: opus
effort: high
permissionMode: bypassPermissions
tools: Bash, Read, Grep, Glob      # no Agent → cannot spawn
disallowedTools: Edit, Write        # verifiers and reviewers only
maxTurns: 60
```

Without this, the orchestrator spawns `general-purpose`, whose model defaults to `inherit` = **Fable**, and the Opus requirement is violated silently on turn one.

Six contracts: **dispatcher**, **verifier**, **meta-verifier**, **diagnostician**, **implementer**, **consensus-reviewer**.

The **dispatcher** reads its verdict from `manifest.json` (`gate.verdict` plus the status state), **never** from `taskfmt run --wait`'s exit code — that exit code is 0 only on `GOAL_MET`, while `is_promotable` also accepts `IDLE` and `GOAL_CLEARED_ERROR`, so reading it reports FAIL on runs the gate passed. It always passes `--repo <ledger.repo_url>` explicitly; `run --exp <id>` with no `--repo` under `--auto` mints a brand-new repository per task. Every `taskfmt` invocation runs from the repository root (`Ctx::load` reads a relative `experiment.toml`).

### 4.4 Completion authority

Three layers, in decreasing trust:

1. **`taskfmt selfhost verdict --final`** — recomputes the result from artifacts: frozen-set hashes, run manifests, GitHub push history, verdict records, and the hash-chained ledger. Exits 0 only if everything checks. This is the authority. The operator or CI re-runs it out of band.
2. **A deterministic `Stop` hook** running `taskfmt selfhost stop-gate`, which blocks the turn while the ledger is unproven. Note the 8-consecutive-block override cap — this prevents premature exit, it is not an engine.
3. **The `/goal` condition**, which only tests that layer 1's sentinel line was printed by a real command in the current turn.

### 4.5 The `/goal` condition

v1's condition ("Done when the ledger records…") was unevaluable: the evaluator runs no commands and reads no files, and a multi-hour transcript is compacted long before the consensus round. It also risked an **Impossible** verdict clearing the goal after a few hundred honest failures.

```text
/goal Drive the self-host proof defined in docs/SELFHOST-VERIFICATION-PLAN.md.
Each turn, run `taskfmt selfhost step`, show its full stdout in the transcript,
and delegate the single NEXT_ACTION line it prints to the subagent it names.

DONE only when, in this same turn, you have run
`taskfmt selfhost status --assert-proven`, its output is shown in the
transcript, it exited 0, and its last line is exactly:
  SELFHOST-PROVEN tag=<t> chains=<k>/<k> verifiers=<n>/<n> cheatseed=0 consensus=clean
That line is emitted by the binary from the ledger. Never type, quote, predict
or paraphrase it; only a real command run in this turn counts.

NOT IMPOSSIBLE: gate FAILs, verifier FAILs, consensus dissents, crashed
containers and environmental faults are the normal path of this work. None of
them makes this condition impossible. On any failure, run
`taskfmt selfhost record`, delegate a diagnosis, and take the next NEXT_ACTION.

If a command fails for an environmental reason (docker, gh, op, network, disk),
run `taskfmt selfhost env-fault "<reason>"` and continue. Never ask the
operator anything.
```

Behavioral rules (delegate everything, Opus only, distinguisher required) are **not** in the condition — the evaluator cannot check them and every extra clause dilutes what it can. They live in the agent definition and in `step`'s output.

### 4.6 Surviving compaction

The ledger is durable **data**; what compaction destroys is the orchestrator's operating contract. Three layers, weakest last:

1. **`step`'s stdout re-emits the invariants every turn.** A rule that arrives in a tool result every turn cannot be compacted out of relevance. This is the load-bearing mechanism because it depends on nothing unverified.
2. **Root `CLAUDE.md`** gains a short permanent block: delegate everything; the frozen set is fenced; after any compaction or resume run `taskfmt selfhost brief` before any other tool.
3. A `SessionStart` hook with `matcher: "compact"` whose stdout is `taskfmt selfhost brief`. `SessionStart` is the only event whose stdout is injected as context; its reliability is contested, hence layers 1–2 carry the weight.

Set `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE=50` — compact early and often, which is cheap once the above holds. Context overflow that auto-compaction cannot clear **clears the goal**, which is what the supervisor exists for.

### 4.7 The supervisor

A session cannot supervise itself, and every goal-clearing fault ends with "control returns to the operator". `taskfmt selfhost supervise` runs **outside** the session: polls the ledger heartbeat; on staleness reads the fault record; relaunches `claude --continue --agent selfhost-orchestrator` (the goal is restored on `--continue`) for transient faults; and for genuinely human-gated faults writes an unmissable record and waits. This is what turns "never stop" from a wish into a property.

---

## 5. Repository layout

```text
selfhost/
  README.md            what this is; NOT source of truth
  AGENTS.md            the meta-substrate protocol (§12)
  CLAUDE.md            symlink -> AGENTS.md
  goal.md              the /goal condition
  contracts/           symlinks -> ../.claude/agents/*.md
  tasks/TASK-1NN/      README.md, verify.toml, decisions.md
  state/               ledger.jsonl, runs/<cycle>/TASK-1NN/progress.md, blocked.json   [gitignored]
  reports/<cycle>/     verdicts, diagnoses, consensus rounds                           [gitignored]
.claude/agents/        selfhost-orchestrator, dispatcher, verifier, meta-verifier,
                       diagnostician, implementer, consensus-reviewer
experiments/references/TASK-0NN/    reference solutions (outside experiments/tasks/**)
```

`selfhost/state/` and `selfhost/reports/` **must be gitignored**. The gate's changed-file enumeration includes untracked files, so orchestrator-written artifacts would otherwise report `OUTSIDE` and fail every meta-task's scope check; whitelisting them instead would let implementers write the ledger.

Meta-task `progress.md` files live at `selfhost/state/runs/<cycle>/TASK-1NN/progress.md` — **outside** the package directory, so the package can protect itself via `forbidden_paths` (§12), and named exactly `progress.md`, because `.gitignore`'s bare `progress.md` matches at any depth while `progress-TASK-101.md` would be committed.

---

## 6. The ledger

**Hash-chained JSONL, not a rewritten JSON document.** One record per line, each carrying `prev_hash` = SHA-256 of the previous record, written `O_APPEND` + `fsync`, with `git commit -s` per append so the history is externally auditable. The JSON view is a derived projection.

Two reasons this matters. First, crash-safety: the only JSON writer in the crate truncates in place with no temp file, no rename and no fsync, so a crash mid-write destroys the only durable state. Second, tamper-evidence: the consensus round's process reviewer reads the ledger, and an orchestrator with Bash on the same tree can rewrite a document called "append-only".

Per **attempt**, provenance is mandatory — without it §9 is remembered rather than decidable:

```
task, cycle, run_id, run_dir, repo_url, clone_sha, base_sha, result_sha,
gate{verdict,exit,last_line,head}, status_state, container,
harness_sha, template_sha, goal_prompt_sha256, taskfmt_binary_sha256,
image_digests, package_tree_hash, distinguisher, fault_class,
metrics{gate_pass, false_done, leaf_claim_accuracy, scope_violation, instruction_violations}
```

`verified`, `cursor` and `repo_url` are **per-cycle**, nested under the cycle record. In v1 they were top-level, so a green-field restart either resumed mid-chain against a new empty repository or contradicted its own "old entries are retained". `cursor` is **always derived** from `verified`, never stored — two sources of truth disagree after a crash.

SHAs orphaned by a reset are marked `orphaned`; a verifier refuses to run against an orphaned SHA rather than failing it (a force-push can make it unfetchable, which would look like a defect).

---

## 7. The outer loop

### 7.1 Every outcome, not two

v1 defined branches for `gate FAIL` and `verdict FAIL`. The real outcome space is larger, and each of these was an undefined transition:

| Outcome | Handling |
|---|---|
| dispatch returns `Err` before any gate exists (clone, lint abort, selfcheck refusal, secret resolution, prereq timeout, pane/idle/prompt timeout) | `DISPATCH_ERROR` → §8. The dispatcher contract returns `{outcome, error_class, run_dir?}` with `run_dir` optional |
| gate PASS but status not promotable (`AGENT_EXITED`, `CONTAINER_STOPPED`, `KILLED_TIMEOUT`) | not a pass. The condition is `is_promotable(status) && gate.passed()` |
| `KILLED_TIMEOUT` | `docker stop` the container **before** gating — the kill path only sends `/goal clear` and leaves the agent writing into the bind mount the gate is about to read |
| promote refuses or the push is rejected | `PROMOTE_ERROR` → §8. Never an agent-work defect |
| verifier crashes, times out, or returns unparseable output | `ABORT`, not FAIL. Re-spawn once with identical inputs — the one legitimate no-distinguisher retry. A second `ABORT` is an environment fault |
| gate exit 70 (internal) | recorded like every other failure, never propagated out of the loop before the record is written |

### 7.2 The cycle

```
0. RESUME: derive cursor from the ledger; never act on a remembered cursor
1. ensure repo (green-field create if the cycle has none)
2. PREFLIGHT probes: docker info · gh auth status · op whoami · gh api rate_limit · disk headroom
3. FROZEN-SET check (§3.1)
4. NO-OP GUARD: refuse to dispatch if the fresh clone already satisfies this task's
   focused suites at base (a selfcheck polarity probe on the clone)
5. DISPATCH cursor into a fresh container; record the intent BEFORE any push
6. gate + status; if not (promotable && pass) → §8
7. PROMOTE; if refused → §8
8. VERIFY: fresh Opus verifier against the pushed SHA
9. if verdict FAIL/ABORT → §8
10. record verified[cursor] = pushed SHA; GC superseded containers; cursor = next
11. cursor past TASK-007 → §11
```

**Step 4 is not paranoia.** Without it, a crash between push and ledger-write leaves the work on `main` with no record; the replay's fresh clone then already contains the finished work, the trusted overlay is a no-op, `base_sha == clone_sha`, the focused suites are green at base, the agent does nothing, and **the gate passes on a no-op run** — silently fabricating a data point in a project whose entire output is a claim about agent behavior.

**Step 5's intent record before the push** is what makes that crash window recoverable: on resume, compare `origin/main` against `verified[cursor-1]`.

---

## 8. Fault taxonomy and recovery

v1 gave the diagnostician a three-way classification over a domain with at least six cases, so a GitHub outage became "a harness defect" and an implementer was dispatched to fix it. Five classes:

| Class | Meaning | Response |
|---|---|---|
| **agent-work** | package and harness sound; the model did it wrong | a legitimate observation. Replay with a `reseed` distinguisher, capped at 3 per task, then escalate the class |
| **package** | the package is defective (contradictory decisions, unsatisfiable AC, whitelist excludes a required file) | fenced repair under §3.2 — green-field cost, `AUTHORIZED-REPAIR`, independent review |
| **harness** | a `taskfmt` bug, a gate hole, a container problem | meta-task under §12 |
| **environment** | docker, gh, op, network, disk, rate limit, model unavailable | **never a meta-task.** Backoff-and-retry with distinguisher `retry-after-environment: <probe>`; on persistence, `NEEDS_OPERATOR` |
| **indeterminate** | cannot be established | a second diagnostician with a different framing, before any code change |

### 8.1 `NEEDS_OPERATOR` — reporting without asking

"Never ask" is achievable; "never be blocked" is not — a session cannot repair its own auth, credit balance, or disk. The correct separation is *asking* (blocking on an answer) from *reporting* (non-blocking):

`taskfmt selfhost env-fault` writes `selfhost/state/blocked.json` naming the failed probe and the exact remediation command, keeps polling that probe with exponential backoff capped at ~15 minutes, and **resumes automatically the moment it passes**. The supervisor (§4.7) surfaces it. Nothing is asked; nothing spins; a ten-second `op signin` clears it whenever the operator next looks.

The faults that need this: 1Password session expiry (default 30 min on macOS — and `op read` runs per dispatch across a 4–6 hour chain), `gh` token scope, Docker daemon death, disk exhaustion, rate limits, credit balance, host sleep.

### 8.2 Anti-spin

**The distinguisher is machine-derived, not narrated.** It is the tuple `(taskfmt_binary_sha256, image_digests, harness_sha, package_tree_hash, reseed_counter)`. Two attempts with the same tuple are the forbidden case regardless of what the record claims — which closes the case where a fix was compiled but never installed, or the images were never rebuilt.

**The mode switch has a reachable exit.** v1's rule suspended dispatch until "a diagnostician reports a materially different root cause" — but a diagnostician's input is run artifacts, and no dispatch means no new artifacts, so the exit could never fire. Now: three consecutive attempts with diagnoses in the same class suspends dispatch until **a meta-task lands whose fix touches a file in the failing class**, and dispatch resumes immediately on that landing.

### 8.3 Revoking a verdict

`verified` was monotone in v1, so when a TASK-005 verifier surfaced a defect that originated in already-verified TASK-003 work, the loop replayed TASK-005 forever against a poisoned base. The diagnostician now emits `earliest_bad_task = k`; the driver drops `verified[k..]`, rewinds the cursor to `k`, and hard-resets to `verified[k-1]` — independent of where the fix lands.

---

## 9. Replay scope — deny by default

v1 listed nine trigger paths. Three of its own four seed tasks matched no row, several fixes matched two rows with no precedence, and it misclassified `lint.rs`/`taskfile.rs`/`verifycfg.rs` as having no run semantics — when lint gates dispatch (`cmds/run.rs:157-166`), `verifycfg` is *the* gate config parser (`gate.rs:256`), and `taskfile` drives the progress check (`gate.rs:532-548`).

**The rule is now deny-by-default.** Any change under `harness/src/**`, `harness/images/**`, `harness/goal-prompt.md`, `experiment.toml`, `experiments/**` or `reference/**` forces a **green-field restart at TASK-001 on a new repository**, except:

| Exception | Replay |
|---|---|
| changes confined to `selfhost/**`, `docs/**` | failing task only |
| changes confined to `harness/src/cmds/selfhost.rs` + the ledger types in `runstate.rs` | failing task only — the orchestration tool touches no run or gate semantics |
| tests that only **add** coverage (no existing test edited or removed) | failing task only |

Where a change matches more than one rule, **the most invalidating outcome wins**.

**Two standing obligations, both missing from v1:**

1. **Every fix touching `harness/**` ends with `cargo build --release && install && taskfmt preload && taskfmt build-images`.** The gate is baked into the images. Without a rebuild, the host gate runs the new engine and the container runs the old one, the agent's two-phase endgame converges on the wrong oracle, and the fix procedure violates §9's own rationale. The resulting binary hash and image digests are the distinguisher (§8.2).
2. `reference/task-template/verify.toml` and `harness/goal-prompt.md` are in the frozen set. The template's `AGENTS.md` **is** the protocol for every run — none of TASK-001..007 ships one, so it is injected at dispatch — and `goal-prompt.md` is the injected `/goal`.

For `cursor == TASK-001`, always green-field (delete and create), never reset in place — per the operator's requirement that a new repository be discarded entirely.

**Reset mechanics.** The harness has **no force-push at all** (`ops/git.rs` exposes only `push` and `push_upstream`), so v1's reset was unimplemented. `taskfmt selfhost reset --repo <url> --to <sha>` must implement it atomically: `push --force-with-lease origin <sha>:main`, guarded so the repo name starts with the configured `repo_prefix`; **rewrite `experiment.json`** dropping every task at or after the reset point; and record the orphaned SHAs. Skipping the second step is silent corruption: `experiment --resume` filters on `task.pushed`, so the task just reset is **skipped**, and if it was the only pending one the run reports success over a commit that no longer exists.

---

## 10. Verification

### 10.1 Independence is structural

A verifier is spawned with a fresh context and given **the repository URL, the pushed SHA and the run's `base_sha`** — nothing else. It must not receive the implementer's transcript, the container's `progress.md`, the gate log, the run directory, or the dispatcher's summary. It clones the repository itself into its own scratch directory and runs every acceptance command itself.

Channels v1 left open, now closed:

- **The run directory.** Verifiers run with no access to `experiments/runs/**`; a verdict whose transcript shows a read of a run path is void.
- **The ledger.** Not readable by verifiers.
- **Prompt wording.** The verifier prompt is a pinned template file; the rendered bytes are recorded in the ledger.
- **Package drift.** `verified[]` entries are 4-tuples `(experiment_sha, taskformat_sha, package_hash, image_digest)`, and the verifier checks out the package at `taskformat_sha` itself — not the current worktree, which may have been repaired since.
- **Diff base.** The verifier diffs against the run's `base_sha` (the trusted overlay commit), **not** the previous verified SHA. The pushed range contains a harness-authored `planner: <TASK> trusted material` commit; diffing against the predecessor scores planner-supplied tests as out-of-scope agent work.

Model-family correlation is closed on the experiment substrate (GLM-5.3-Flash implements, Opus verifies) and open on the meta substrate (Opus both sides) — stated, not hidden.

### 10.2 What the verifier checks

The host gate is necessary and not sufficient: the recorded TASK-003 run passed 11/11 gate checks while overriding a fixed decision, and wrote so in plain text. The verifier exists for that class of gap.

1. Clone fresh at the recorded SHA; refuse if `orphaned`.
2. Run every `AC-*` evidence command; capture output. A command that cannot run is `NOT_RUN` → FAIL, never an inferred pass.
3. Confirm every `R-* (MUST)` and `(MUST NOT)` against the diff from `base_sha`.
4. **Decision conformance** — every `D-*` the task's scope touches. This is the gate's blind spot and the verifier's main job.
5. Confirm the artifact works: build, run the real entry points, inspect gallery output where produced.
6. Return a **schema-validated** verdict: `overall ∈ {PASS, FAIL, ABORT}`, per-AC array, per-R array, decision-conformance list, and on FAIL the single most decisive piece of evidence.

**Reward-hacking checks are mechanical, not judged.** Weakened tests, deleted tests, added `#[ignore]`, added lint suppressions, swallowed errors and out-of-scope changes are `instruction_violations` greps computed by the driver. The verifier judges only what a grep cannot.

**The verifier runs inside the `harness-*` image** with the same prereq stage and **no `op`/`gh` credentials in its environment. Reason: a host PASS is not the in-container gate's PASS, TASK-004+ needs Docker and testcontainers, and building agent-authored Rust on the credentialed operator host is both an environment mismatch and an unnecessary exposure.

Long commands (cold Rust builds, 102-test testcontainers suites, `status --wait` on a 35-minute run) exceed the Bash tool's 10-minute ceiling. Raise `BASH_MAX_TIMEOUT_MS` and require `run_in_background` + poll; write full build/test output to a file and grep it, so only the structured verdict enters context.

### 10.3 Meta-tasks get a verifier too

v1's implementer authored the fix, could author its own `verify.toml`, and declared itself done — reinstating exactly the self-verification the project exists to eliminate, on the path that carries every harness fix and every fenced repair. The **meta-verifier** has the same rules: fresh context, clones this repo at the commit the implementer pushed to `main`, runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `taskfmt selftest`, the meta-task's ACs, and `diff-check`. Its verdict — not the implementer's — writes `meta_tasks[].verified`.

Additionally, a meta-task's gate must be run by a `taskfmt` built from the **pre-fix** SHA in the meta-verifier's own clone, never by the binary the implementer just rebuilt in the working checkout.

### 10.4 Calibration against known-bad trees

A reviewer that passes a tree known to be defective has demonstrated zero discriminative power, and its PASS on the real tree carries no information. **Before any contract's verdict is trusted, it must FAIL these:**

1. `experiments/runs/20260828-175916-zai-flash-TASK-003/workspace` — gate PASS with a documented decision violation. This labeled fixture already exists on disk; it is the cheapest high-value check in the plan.
2. A deliberately sabotaged tree: one added `#[ignore]`, one deleted trusted test.

Calibration results are recorded in the ledger and re-run whenever a contract's prompt template changes.

---

## 11. Terminal proof

Reaching a verified TASK-007 does not end the goal. It unlocks a five-part terminal gate, in order.

**11.1 Freeze.** `taskfmt selfhost freeze` tags the tree and hash-pins the frozen set and image digests. Images are rebuilt **from the tag** — `taskfmt build-images` is a mandatory step, and `run` refuses on a host/image version mismatch. Any subsequent change to the frozen set voids the tag and resets k to 0.

**11.2 Cheat seed.** One Harbor-style hack-prompted seed per task against the frozen object; require `cheat_seed_gate_pass == 0`. Until this is measured, a green chain is evidence that the gate is *passable*, not that the format works — and the project's own findings make this a precondition of trusting any format comparison. A non-zero result is a harness finding: fix, re-freeze, restart. If any seed passes, adopt the host-only `verify.toml` overlay (invisible to the container) as an additional gate layer.

**11.3 k ≥ 3 held-out chain replays.** Each on a fresh repository, each **dispatched from a separate read-only clone of the frozen tag** so a mid-replay edit cannot reach it, each unattended with zero interventions. Plus n ≥ 5 replays for TASK-002 and TASK-003 from their verified predecessors, reported as pass rate ± stddev.

"No interventions" is not self-attested: record the replay window, and have the process reviewer check the disposable repo's push events for force-pushes and the run directory for extra runs inside that window. A reset leaves evidence.

**11.4 Independent per-task verification of every replay.** 7 × k fresh verifiers, per §10.

**11.5 Bounded consensus.** v1 required unanimity from a reviewer explicitly told to "find the reason this does not count as proof", while the project's own documentation lists permanent grounds for dissent that will still be true after any replay (the oracle has never run; TASK-001 cannot be proven RED at its base). That is a livelock as the goal's termination condition — and it is compounded by the known bias that a reviewer prompted to find gaps will find them.

Instead:

- A **fixed rubric**, plus a **pre-registered, ledger-recorded list of accepted limitations** that are explicitly not grounds for dissent.
- A dissent counts **only if it cites a reproducible falsifying command with its output**. The driver re-runs the cited command; if it does not reproduce, the dissent is recorded and overruled in the ledger.
- Reviewers are blind to prior verdicts and to the round's stakes.
- **At least one reviewer from a different model family** — three Opus reviewers on one evidence set, with prompts from one author, produce correlated agreement, not independent confirmation.
- The **process reviewer reconstructs the chain from independent evidence** (the repo's commit and push history, plus run manifests) and FAILs on any mismatch with the ledger — it does not read the ledger as fact.
- **The orchestrator does not get a vote.** It delegated everything, so it has no first-hand evidence, and it is the party whose goal terminates on PASS. It attests that the process ran; that is not a verdict.

---

## 12. The meta-task substrate

`reference/task-template/AGENTS.md` assumes a container. `selfhost/AGENTS.md` is its host delta — with a sibling `CLAUDE.md` symlink, as the repo-wide rule requires.

| Template clause | Host replacement |
|---|---|
| `/task` ro, `/work` rw, `/progress` rw | the package dir, the **`main` checkout**, `selfhost/state/runs/<cycle>/TASK-1NN/progress.md` |
| `/task` read-only *by mount* | no mount exists. **The package lists its own directory in `forbidden_paths`**, so any edit to it fails the gate. The package protects itself |
| gate is the baked-in binary | `harness/` is the artifact under edit. Gate is `cargo run -q --manifest-path harness/Cargo.toml -- verify`, never the `PATH` binary |
| `$TASKFMT_BASE` | the spawner exports four vars so the gate behaves as in a container: `TASKFMT_ROOT`, `TASKFMT_TASK_DIR`, `PROGRESS_FILE`, `TASKFMT_BASE` (the branch tip when the implementer was spawned; never a SHA in `base_ref`, which goes stale and lints clean) |
| — | **Never `git commit`, `git stash`, `git push`.** A commit empties the base diff and makes `scope` pass vacuously. The orchestrator commits after the gate |
| — | Never write `selfhost/state/**` or `selfhost/reports/**` |
| — | **New-test rule:** a focused command must name a **new `--test` target file**. A new filter inside an existing target exits 0 at base ("0 filtered out") — a silently green baseline. Only a missing target is genuinely red |
| — | Commands use `--manifest-path harness/Cargo.toml`; the workspace is not at the repo root, and `cd harness && …` defeats the linter's `cargo test` spec matching |
| `NEEDS_REPLAN` triggers | add: the fix requires editing a fenced path and this package carries no `AUTHORIZED-REPAIR:` line for it |
| protocol steps 1–8, progress grammar, prohibitions, stop conditions, turn signal, final report | **unchanged, verbatim** — identical transcript grammar is what makes the substrates comparable at all |

**One implementer at a time, in the `main` checkout.** The repository works on `main` only (root `AGENTS.md`), so meta-tasks do not get the fresh-clone isolation that experiment tasks get — a deliberate, stated exception to "one task = one fresh container = one fresh clone", and part of why §0 records that the meta substrate is not evidence about the research question. Two consequences follow and both are load-bearing. **Implementers are strictly serialized**: the orchestrator's one-subagent-per-turn rule (§4.2) is what enforces it, and a second concurrent implementer would put another agent's edits into the first one's changed-file set. **`selfhost/state/` and `selfhost/reports/` must be gitignored** (§5), because the gate's changed-file set includes untracked files and the orchestrator writes there continuously — otherwise every meta-task fails `scope` on artifacts its implementer never touched.

**Selfcheck is mandatory on this substrate.** `taskfmt selfcheck <pkg> --workspace <fresh clone at base> --base <sha>` must PASS before an implementer is spawned, and the report goes in the ledger. Otherwise nothing enforces a red baseline: selfcheck is opt-in at dispatch and the `baseline` lint rule is a WARN restricted to `kind: feature|bugfix`, so a meta-task with a green baseline lints clean, gates clean, and is unfalsifiable.

**Enforcing the fence mechanically**, three layers:

1. `forbidden_paths` per package — exact-prefix on the changed set, zero cost when untouched:
   `["experiments/tasks", "reference/task-template", "docs/research/RESEARCH-FINDINGS.md", "selfhost/tasks/TASK-1NN", "selfhost/state", "selfhost/reports"]`. For an authorized repair, drop the fenced prefix and re-add the other packages **by name**.
2. `allowed_globs` must equal `expected_paths` (lint-enforced), so an unlisted fenced path fails `scope` independently.
3. `taskfmt selftest` gains a repo-wide check: every `selfhost/tasks/*/README.md` carries the disclaimer sentence verbatim, and its `verify.toml` forbids `experiments/tasks` unless the README carries an `AUTHORIZED-REPAIR:` line. This is what turns the fence from prose into a failing check.

Every `selfhost/tasks/*/README.md` carries this verbatim under `## Goal`:

> This is a meta-task. It is scaffolding for proving the project, not part of the research object. `experiments/tasks/**` is the source of truth; this package is not.

---

## 13. Work items — three categories

v1 forced every known blocker into a package shape. Two of its four could not hold one, so the plan needs two more categories.

### 13.1 Meta-task packages (gated by `taskfmt verify`)

| ID | Kind | Goal | RED baseline |
|---|---|---|---|
| **TASK-101** | bugfix | Confirmation results are honored and `--auto`/`--yes` reach every mutating step. `repo create`, `repo delete` **and `promote`** all discard their confirm result — `promote` is the one command that writes `origin/main`, and v1 missed it. Also unshadow the subcommand-local `--yes` | new `--test consent_matrix` target absent at base (rc 101) |
| **TASK-102** | feature | `taskfmt selfhost` driver and ledger per §4.1/§6 | new `--test selfhost_ledger` target |
| **TASK-103** | feature | Reference solution for TASK-002 under `experiments/references/`, plus a committed base workspace fixture; the **oracle phase runs green for the first time** | `selfcheck --reference` non-zero at base |
| **TASK-104** | feature | Container-mode selfcheck, so TASK-001 stops returning NOVERDICT and the oracle becomes reachable for the whole corpus | new test target + a selfcheck exiting 69 at base |
| **TASK-105** | bugfix | Fix `git::changed_files`' unfiltered `**/.gitignore` enumeration (§14 item 2) | new `--test scope_ignore_matrix` target |
| **TASK-106** | bugfix | Harness robustness for unattended operation: replace `fail_prereqs`' `process::exit(1)` with a recorded error; wrap the `wait_and_gate` `?`; add `--kill-after` to `experiment`; hoist the duplicate-entry `retain`; `repo delete` prefix guard in Rust | new `--test unattended_matrix` target |

TASK-101 and v1's SELF-004 were the same task — SELF-004's defects are *why* SELF-001's goal was unmet, on the same files. Merged.

### 13.2 Ledger decision records (evidence = dispatched runs)

**The D-032 amendment.** `experiments/tasks/TASK-003/decisions.md:110` requires all fields non-empty except Password; `trusted/…/screen_create_form_test.rs:62-71` types only a name, presses Enter, and asserts `SaveConnection` is `effects[0]`. The test is unsatisfiable against the decision. This is a genuine defect in the research object — and it has **no discriminating gate command on any available substrate**: lint has no semantic view of decisions, selfcheck polarity is red either way (the form does not exist yet at base), a `grep` for the new wording proves only that a string was typed, and only the never-run oracle phase can see it.

So it is a decision record, not a package. **Polarity is fixed in advance, before the loop runs**, because otherwise the loop will resolve it the way the one recorded run did — by overriding the decision — and the project will have retro-fitted its spec to the output of a run: **the `D-*` is authoritative and the trusted test is corrected to match.** The amendment is authored by a diagnostician spawned **without** the failing run's diff, given only the two conflicting texts, and is accepted only on **two consecutive green dispatched TASK-003 runs**.

### 13.3 Ledger milestones (outer-loop outcomes, no gate)

- **Unattended smoke cycle** — a trivial `TASK-9xx` smoke package (write a file, one focused grep) dispatched with `taskfmt run --task <path>`, green with zero interventions. v1's SELF-001 made "the whole seven-task chain runs unattended" a task's acceptance criterion, which is circular and costs hours per attempt; a real smoke corpus makes it a minutes-long check.
- **First full unattended chain** — 7/7 gate PASS with no intervention.
- **Freeze**, **cheat seed clean**, **k=1, k=2, k=3**, **consensus clean**.

---

## 14. Pre-flight — must exist before the first dispatch

Ordered by what breaks first. Items 1–5 are **operator work, done and merged before the session starts**: the orchestrator must never be its own bootstrap, and v1 put the ledger — its only durable state — inside a task the orchestrator was supposed to implement while modifying the harness it depends on.

1. **Install the toolchain.** `taskfmt` is not on `PATH` (only `harness/target/{debug,release}/taskfmt`) and **zero `harness-*` images exist on this host**. `cargo install --path harness && taskfmt preload --auto && taskfmt build-images --agent claude --auto`.
2. **Fix `git::changed_files`.** `ops/git.rs:183` enumerates `:(top,glob)**/.gitignore` with exclusions deliberately bypassed. On this worktree that reports 14 Claude-plugin cache files under `experiments/runs/*/agent-home/` as `OUTSIDE`, so **`taskfmt verify` fails scope today and no meta-task can ever gate green.** Fix: intersect that enumeration with paths not ignored by the **base commit's** rules, so an executor-added `.gitignore` still surfaces while pre-existing ignored trees do not. Interim: relocate `paths.runs_dir` outside the repo. (This is the mirror image of the known scope hole — over-reporting instead of hiding — and is in neither the tamper matrix nor the backlog.)
3. **`.gitignore` `selfhost/state/` and `selfhost/reports/`.**
4. **Build `taskfmt selfhost`** with the §4.1 scope and the §6 ledger. Includes `reset --to` with `--force-with-lease` and the `repo_prefix` guard, `invalidate`, `gc`, `freeze`, `diff-check`, `env-fault`, `supervise`, `verdict --final`.
5. **Make secret resolution survivable.** A 1Password **service-account token**, or resolve `env_secret` once per chain and reuse the `SecretEnvFile`. `op read` runs per dispatch; a locked vault blocks on a GUI dialog and an expired session hard-errors, and operator action is then the only recovery — which is precisely what "never ask" cannot cover.
6. **`.claude/agents/*.md`** — seven definitions, each pinning `model: opus`, `effort: high`.
7. **`.claude/settings.json`** — the `Stop` hook, the `SessionStart:compact` hook, and env: `BASH_MAX_TIMEOUT_MS`, `CLAUDE_CODE_GOAL_CHECKIN_MINUTES=10`, `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE=50`, `CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH=2`.
8. **Root `CLAUDE.md`** gains the §4.6 invariant block.
9. **`selfhost/AGENTS.md` + `CLAUDE.md` symlink** — hand-authored and human-reviewed, since it defines the protocol that gates everything after it.
10. **Raise `runtime.kill_after_min` to ≥ 180.** It is 90 and not overridable on `experiment`; TASK-001 took 42 min and TASK-002 31 min, and TASK-005..007 are strictly larger (73→102 cumulative tests) on 2 CPUs.
11. **Calibrate every contract** against the §10.4 known-bad trees.
12. **Smoke corpus** (§13.3).

---

## 15. Operator preconditions

Docker running · `gh auth refresh -s delete_repo` · 1Password service-account token exported · this repo clean and on `main` · **workspace trusted** (`/goal` is gated by the same trust rule as hooks) · disk headroom asserted · `caffeinate -dimsu` around the session · `taskfmt selfhost supervise` started **before** the session.

Launch: `claude --agent selfhost-orchestrator --permission-mode bypassPermissions`, then paste §4.5.

Observation: `taskfmt selfhost status` prints a bounded table; the `Stop` hook appends a heartbeat each turn, which is also what the supervisor polls.

---

## 16. Costs, resources, risks

- **Wall clock.** ~35 min per recorded task, more for TASK-005..007. One clean chain is half a day; k=3 plus n≥5 on two tasks is several days of machine time.
- **Containers are never reaped by the harness** — deliberately, so failures stay inspectable. With no operator that means seven `--privileged` containers per cycle, each with an inner dockerd, a postgres, a 4 GB reservation and a full Rust target dir. Host exhaustion mid-cycle is the realistic outcome, and the resulting dispatch failures present as harness bugs. `taskfmt selfhost gc --keep-last` must reap containers and run dirs for superseded attempts, and a max-live-containers invariant is required.
- **Opus verification is the dominant token cost**, and headed mode has no budget cap. Serialize verifiers rather than fanning out seven. If cost control matters more than the single-session framing, run the model workers as `claude -p --output-format json --json-schema … --max-budget-usd X` under the driver: print mode is the only mode with a working budget ceiling, and it also gives §10.2's schema-validated verdict for free.
- **Destructive authority.** §10 of v1 claimed deletion was scoped to the configured prefix. It is not: `repo delete` takes an arbitrary name with no prefix check and discards its confirmation. The guard belongs in Rust with a `selftest` check (TASK-101/106) — it is currently the plan's only named safety property and it is fictional.
- **The honest risk remains convergence by weakening.** The fence (§3), the inverted cost gradient, `diff-check`, the five-way fault taxonomy, the pre-fixed D-032 polarity, calibrated verifiers and the freeze are all aimed at it. It stays the thing to watch.

---

## 17. Deferred, deliberately

Extending reference solutions to all seven tasks (TASK-103 covers TASK-002 only); the research metric suite beyond the six in §6; `false_done_vendor`; the Codex substrate (the `codex-default` profile has no `env_secret` and `taskfmt codex-login` is invoked by the entrypoint but does not exist as a subcommand); and the format ablations in the research backlog. None of these gate §1.
