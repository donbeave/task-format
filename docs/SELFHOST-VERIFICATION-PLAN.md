# Self-Host Verification Plan — proving task-format works

**Status: DRAFT v4 — after three adversarial review rounds (twelve reviewers).**

The plan for proving that `task-format` works: that a `task/v4` package, handed to a fresh agent in a fresh container, produces correct, independently verifiable work — from an empty repository to the finished `pgtui` app.

**The change in v4.** v3 collapsed under its own machinery. Reviewers found four defects that stopped it running at all, seventeen dangling references, and eighteen internal contradictions — nearly all of them at the seam between "prove the project" and "build an autonomous loop that proves the project". v4 separates those into three phases, so most of the seam disappears rather than being specified.

| Phase | What it delivers | Machinery needed |
|---|---|---|
| **1 — Attended chain** | one verified chain, operator in the loop | almost none: existing `taskfmt`, host verifier subagents, a plain log |
| **2 — Unattended loop** | the operator's literal brief: Fable `/goal`, Opus subagents, self-driving | the driver, the fence, the contracts |
| **3 — Research claim** | statistical support for the format claim | freeze, repeat replays, cheat seed, consensus |

**Phase 1 is the deliverable.** It takes the project from **zero measured runs** to one independently verified chain, and it is the shortest path to discovering whether anything past TASK-003 works at all — nothing past TASK-003 has ever been dispatched.

Read §0 first; it bounds every claim below.

---

## 0. What this can and cannot prove

| Layer | Self-hosted here? | Evidence about the research question? |
|---|---|---|
| **1. Task package format** (`task/v4`, `lint.rs`) | **Yes** — byte-for-byte the same linter | **Usability and discrimination only — not predictability** |
| **2. Agent protocol** (`reference/task-template/AGENTS.md`) | No — meta-tasks run a forked host protocol (§9) | No |
| **3. Gate** (`taskfmt verify`, `gate.rs`) | **Yes** — the same engine on both substrates | **Usability and discrimination only** |
| **4. Execution harness** (container, herdr, dispatch) | No — meta-tasks run on the host | No |
| **5. Model / provider** | No — meta-tasks run Opus, experiment tasks run `zai-flash` | No |

Rows 1 and 3 say "usability and discrimination" precisely because rows 2, 4 and 5 differ on the meta substrate, so no predictability inference survives. Consensus reviewers may not cite meta-task outcomes as evidence about the format's predictability.

**Verifier isolation is hook-and-convention enforced, not mount-enforced** (§5.1). A host subagent holding `Bash` can read anything the operator can. A hook bypass is therefore a silent-failure mode of the Phase 1 claim, and it is stated here rather than hidden behind the word "structural".

---

## 1. The three claims

**Phase 1 — the deliverable.**

> One complete chain exists from an empty repository to a finished `pgtui`: all seven experiment tasks passed the gate in sequence on one repository, and each was independently confirmed by a fresh host verifier that cloned the pushed SHA itself and ran the acceptance commands itself. The operator was attended throughout. Run-to-run variance is unmeasured.

**Phase 2 — the operator's brief.**

> The same chain, produced by a Fable `/goal` session that delegated every unit of work to Opus subagents, recovered from its own failures, and required no operator intervention between start and terminal.

**Phase 3 — the research claim.**

> From a frozen, hash-pinned research object, k chains were replayed from read-only clones of the tag with images pinned by digest; the observed chain pass rate is r; a pre-registered cheat seed scores zero; and a bounded consensus round found no dissent carrying a reproducible falsifier.

### 1.1 On k, and on the variance data

A **counted replay** contains **zero reseeds, first attempt only**; the ledger records attempts per task.

v2 required "k ≥ 3 **consecutive** clean chains" and justified it with per-task p ≈ 0.5–0.6. Those do not compose: at that p, three consecutive 7/7 chains has probability 0.5²¹ ≈ 4.8×10⁻⁷ to 0.6²¹ ≈ 2.2×10⁻⁵ — an unreachable criterion. Phase 3 therefore reports a **rate over k independent chains**, not a run of consecutive successes.

And the p was invented. From the manifests:

| Task | runs | outcome | durations |
|---|---|---|---|
| TASK-001 | 2 | **both PASS** | 2.6 min, 3.8 min |
| TASK-002 | 2 gated + 1 no gate | 1 pass, 1 fail | 16.3 min both |
| TASK-003 | 2 | 1 pass (the D-032 override), 1 fail | 34.7 min, 14.3 min |

Pooled: 4 pass, 2 fail, 1 no-gate. More importantly **these are not experimental runs** — the findings say so, and the package that produced them has since changed (their `AGENTS.md` predates D31/D32/D38; TASK-003's checklist went from 10 leaves to 11). Per-task p is **unknown**.

So: **measure p before choosing k.** n=5 replays of TASK-002 from a fixed predecessor SHA is cheap and sets k. Do not set k from a number derived from debugging runs of a different research object.

---

## 2. The two substrates

| | **Experiment tasks** | **Meta-tasks** |
|---|---|---|
| Location | `experiments/tasks/TASK-001..007` | `selfhost/tasks/TASK-1NN` |
| Subject | a disposable GitHub repo that becomes `pgtui` | **this** repository |
| Executor | Claude Code, `zai-flash`, **in a Docker container** | **Opus / high** subagent, on the host, in the `main` checkout |
| Gate | in-container `taskfmt verify`, then host `taskfmt gate` | `cargo run -q --manifest-path harness/Cargo.toml -- verify` |
| Independent verifier | fresh Opus, clones the pushed SHA | fresh Opus, clones `main` at the implementer's commit |
| Source of truth? | **YES** | **NO** |
| Phase | 1, 2, 3 | 2 onward (Phase 1 fixes are operator-authored) |

Meta-task IDs are `TASK-1NN`, never `SELF-0NN`. `lint.rs:190` requires `^TASK-[0-9]+$` on the frontmatter id, `lint.rs:275` requires the H1 to start `TASK-<digits>`, and `gate.rs:589-590` extracts `TASK-[0-9]+` from both the progress header and the README. A `SELF-` id fails lint, blocks `progress-init` (which refuses on any lint ERROR), and can never satisfy the progress check. Relaxing those would edit **the gate**. The plan changes; the linter does not.

---

## 3. Phase 1 — the attended chain

No driver, no `/goal`, no fence, no ledger schema. The operator runs the existing tooling and spawns verifier subagents by hand. Everything Phase 1 needs already exists or is a one-line config change, except the pre-flight fixes in §3.1.

### 3.1 Pre-flight

| # | Item | Size |
|---|---|---|
| 1 | Install the toolchain. `taskfmt` is not on `PATH` and **zero `harness-*` images exist on this host**. `cargo install --path harness && taskfmt preload --auto && taskfmt build-images --agent claude --auto` (three builds: taskfmt → base → claude) | **½–1 day** — it has never run here; the base installs the Rust toolchain 1.98.0, Node 22, herdr 0.8.2 and `postgresql-client-16` |
| 2 | Move `paths.runs_dir` outside the repository | minutes; evicts 33 GB and removes the scope problem in item 3 |
| 3 | **TASK-105** — fix `git::changed_files`' unfiltered `:(top,glob)**/.gitignore` enumeration (`ops/git.rs:183`), which reports **exactly 14** plugin-cache files under `experiments/runs/*/agent-home/` as `OUTSIDE`. **Item 2 makes this non-blocking for Phase 1**; it remains required before any meta-task gates on this repo (Phase 2) | ½–2 days; git has no primitive for "the base commit's ignore rules" |
| 4 | Secret survivability: a 1Password **service-account token**. `op read` runs per dispatch across a 4–6 hour chain against a 30-minute default session | hours |
| 5 | **TASK-106** — unattended robustness: `fail_prereqs`' `process::exit(1)` (`run.rs:485`) → a recorded error; wrap `wait_and_gate`'s `?` (`run.rs:304`), which escapes before `state.save`; `--kill-after` on `experiment`; hoist the duplicate-entry `retain` (`experiment.rs:162`) above both failure paths; the `repo delete` prefix guard in Rust | 1–2 days |
| 6 | **TASK-101** — confirmation results honored. `repo create` (`repo.rs:26`), `repo delete` (`repo.rs:56`) **and `promote`** (`promote.rs:65`) all discard their confirm bool; `promote` is the one command that writes `origin/main`. Unshadow the global `--yes` | 1–2 days |
| 7 | Raise `runtime.kill_after_min` from 90 to ≥180 | minutes |
| 8 | Copy the calibration fixtures into `selfhost/fixtures/known-bad/` and commit them (§5.3) | ½ day |
| 9 | Add `selfhost/` scaffolding: `README.md`, and a `.gitignore` entry for `selfhost/reports/` | minutes |

Items 1, 2, 4, 5, 6, 7, 8, 9 are Phase 1 blockers. Item 3 is a Phase 2 blocker kept here because it is cheap to do alongside item 5.

**Total: about one working week.**

### 3.2 The chain

Dispatch with **`taskfmt experiment`**, not `taskfmt run`. This matters and v3 got it backwards.

`taskfmt run --exp <id>` **never persists `ExperimentState`** — `state.save` is called only from `experiment.rs:144`, `:157`, `:164`; `run.rs:59` merely *loads* it. So under `run`, recorded state is always absent, `resolve_repo_url` falls through to the create closure, and **every dispatch mints a brand-new empty repository**. Task 2 would then land on a repo containing none of task 1's work — and its gate would pass, because `[focused]` is red at base by construction. That fabricates a chain. v2's "always pass `--repo`" rule was correct; v3 removed it on a false premise.

Two safe dispatch forms:

- `taskfmt experiment --tasks all --auto` — persists state, pins the repo, runs the whole chain. Use this in Phase 1.
- `taskfmt run --task <ID> --repo <url> …` — an explicit `--repo` every time. Required if anything ever dispatches one task at a time.

Per task the operator: runs the chain (or one task), reads `manifest.json` for `gate.verdict`, spawns a verifier (§5), records the verdict in `selfhost/state/ledger.jsonl` (out of tree, §3.3), and moves on. On a failure: diagnose, fix, and restart the chain from scratch on a new repository. Phase 1 does not need `reset --to`; green-field is correct and the harness has no force-push anyway (`ops/git.rs` exposes only `push` and `push_upstream`).

### 3.3 The log

A plain append-only JSONL file **outside the worktree**, at `$XDG_STATE_HOME/taskfmt-selfhost/<repo-id>/ledger.jsonl`.

Out of tree because you cannot `git commit -s` a gitignored path, and an in-tree tracked ledger *would* appear in the gate's changed set once tracked (`git diff <base>` and `--cached` are two of the four enumerations in `git.rs:177-197`). Note the untracked enumeration at `:182` uses `--exclude-per-directory=.gitignore`, so an in-tree **gitignored** ledger would *not* be reported `OUTSIDE` — v3 claimed otherwise, and that claim was wrong. The decision stands on the commit argument alone.

Per attempt: `task, cycle, run_id, run_dir, base_sha, result_sha, gate{verdict,exit,last_line,head}, status_state, container, started, finished, verdict_file, verdict_sha256`.
Per cycle: `repo_url, verified{}, window_start, window_end`. `cursor` is always derived from `verified`, never stored.

`Manifest` already carries `start` (`runstate.rs:53`), which is where §12's timings come from; the ledger adds `finished` and the verifier verdict, which nothing else holds.

### 3.4 What "verified" requires

For a task to count as verified, all three must hold and be recorded:

1. `manifest.gate.verdict == "pass"`.
2. A schema-validated verifier verdict file exists, its SHA-256 is in the ledger, and `overall == "PASS"`.
3. `result_sha` appears in `gh api repos/<owner>/<repo>/commits`.

Condition 2 exists because a verdict retyped by an intermediary is not an artifact. Condition 3 exists because the chain's whole claim is that the work reached `origin/main`.

---

## 4. Phase 2 — the unattended loop

The operator's literal brief. Entered after Phase 1 lands.

### 4.1 `taskfmt selfhost` — the driver

The loop is deterministic; only four steps need judgement (verify, diagnose, implement, consensus). The driver owns the loop, the cursor and the state, and emits **one `NEXT_ACTION` line per call** naming the contract to run next.

Subcommands, all of them, with no others named anywhere in this document: `step` · `record` · `status` · `probe` · `gc` · `env-fault` · `token` · `reset` · `freeze` · `diff-check` · `verdict`.

| Subcommand | Job |
|---|---|
| `step` | emit the next `NEXT_ACTION`; re-emit the invariants (§4.4); refuse if a fenced path is dirty outside an authorized repair |
| `record --verdict-file <path>` | validate, hash and append a verdict; advance the cursor. **Called on success and on failure** |
| `status` | bounded table; `--sentinel` prints the terminal line |
| `probe` | the §6.2 preflight probes; also the fault detector for `env-fault` |
| `gc --keep-last <n>` | reap containers and run dirs for superseded attempts. **Default n=2**; never reaps `selfhost/fixtures/` |
| `env-fault <reason>` | write the fault record and **return immediately** |
| `token mint\|verify` | the repair token (§7.2) |
| `reset --to <sha>` | `push --force-with-lease origin <sha>:main`, guarded by `repo_prefix`; **rewrite `experiment.json`** dropping every task at or after the reset point; mark orphaned SHAs |
| `freeze`, `diff-check`, `verdict --final` | Phase 3 only |

The full surface is a 4,000–7,000-line subsystem — a 37–65% expansion of the 10,793-line crate, 4–8 weeks. The Phase 2 subset (`step`, `record`, `status`, `probe`, `gc`, `env-fault`, `token`, `reset`) is roughly 1,500–2,500 lines.

**Ownership.** `manifest.json` and `ExperimentState` own run identity and repo pinning. The ledger stores only verifier verdicts, fault records, provenance hashes, cycle framing and timestamps.

**One correction v3 made that is right and one that was wrong.** Right: `run` does **not** refuse on a host/image version mismatch — no such check exists anywhere in `harness/src`; it is backlog 25, scheduled here as TASK-107, and Phase 3 depends on it. Wrong: see §3.2 on repo pinning.

### 4.2 The orchestrator — Fable 5, effort high

`claude --agent selfhost-orchestrator --permission-mode bypassPermissions`. Agent definition: `model: fable`, `effort: high`, `disallowedTools: Edit, Write, NotebookEdit`, `tools: Agent(...), Bash, Read, Grep, Glob`.

**Delegation is not enforced by tool removal.** `Bash` writes files — `cat >`, `sed -i`, `python -c` — and argv matching does not see `cd harness && sed -i`, a heredoc, `bash -c "$(…)"`, `git checkout -- <path>` or a path built from a variable. So a `PreToolUse` Bash deny is **defense in depth**, and the real guard is mechanical and decidable: **`taskfmt selfhost step` hard-fails when `git status --porcelain` shows any fenced path dirty at the start of a turn outside an authorized repair.** That cannot be evaded by shell quoting.

Never pass the `Agent` tool's `model` parameter (it overrides frontmatter) and never use `subagent_type: "fork"` (it always inherits the parent model, so a fork runs Fable).

Per turn: `step` → delegate the one `NEXT_ACTION` → await in the same turn → `record` → end. The orchestrator never `Read`s the ledger; only `step`/`status` output enters its context, capped at ≤80 lines.

### 4.3 The contracts

Definitions live in `.claude/agents/*.md` — Claude Code loads subagents only from there, and `general-purpose` defaults to `inherit`, so definitions placed anywhere else mean every subagent silently runs Fable.

**The orchestrator's definition pins `model: fable`. The seven subagent definitions pin `model: opus, effort: high`.**

| Contract | Tools | Writes | Phase |
|---|---|---|---|
| **planner** | Read, Grep, Glob, Bash, **Write scoped to `selfhost/tasks/**`** | the meta-task package, **and commits it** | 2 |
| **dispatcher** | Bash, Read, Grep, Glob | the heartbeat, via `taskfmt selfhost record --heartbeat` | 2 |
| **verifier** | Bash, Read, Grep, Glob | its verdict file | 1 (hand-spawned), 2 |
| **meta-verifier** | Bash, Read, Grep, Glob | its verdict file | 2 |
| **diagnostician** | Read, Grep, Glob | its diagnosis file | 2 |
| **implementer** | Read, Edit, Write, Bash, Grep, Glob | the fix | 2 |
| **blind-reviewer** | Read, Grep, Glob | its verdict file | 2 |
| **consensus-reviewer** | Bash, Read, Grep, Glob | its verdict file | 3 |

**The planner has `Bash` and commits its own package.** v3 gave it `Write` only, and set `TASKFMT_BASE` to the tip *before* the package existed — so the package landed in the gate's changed set, where both its own `forbidden_paths` entry and the scope whitelist rejected it. Every meta-task would have failed its own gate on creation. The planner commits the package, *then* `TASKFMT_BASE` is captured, *then* the implementer is spawned. The planner may never implement the task it planned.

**The blind-reviewer is the actor §7.2 needs** — it authorizes repair tokens and is never shown the failing run's diff. v3 required this role and assigned it to nobody.

The **dispatcher** reads its verdict from `manifest.json` (`gate.verdict`) and from the `status_state` that TASK-106 persists beside it. It must not use `taskfmt run --wait`'s exit code, which is 0 only on `GOAL_MET` (`run.rs:81-85`) while `is_promotable` also accepts `IDLE` and `GOAL_CLEARED_ERROR` (`status.rs:287-289`) — reading it discards passing runs as failures. Every `taskfmt` call runs from the repository root (`Ctx::load` reads a relative `experiment.toml`).

### 4.4 `/goal`, compaction, and the supervisor

The evaluator runs no commands and reads no files, and a multi-hour transcript compacts long before the end. So the condition names **one sentinel printed by a real command in the current turn**, and immunizes against an *Impossible* verdict.

```text
/goal Drive the self-host proof in docs/SELFHOST-VERIFICATION-PLAN.md.
Each turn, run `taskfmt selfhost step`, show its full stdout in the transcript,
delegate the single NEXT_ACTION line it prints to the subagent it names, and
then run `taskfmt selfhost record` with that subagent's result — on success and
on failure alike.

DONE only when, in this same turn, you have run `taskfmt selfhost status
--sentinel` and its last line is exactly one of:
  SELFHOST-CHAIN-VERIFIED cycle=<id> tasks=7/7 verifiers=7/7
  SELFHOST-NOT-PROVEN model=<profile> failed_at=<task> reason=<class>
Both are terminal. Both are emitted by the binary from the ledger. Never type,
quote, predict or paraphrase either; only a real command run this turn counts.

NOT IMPOSSIBLE: gate FAILs, verifier FAILs, crashed containers and
environmental faults are the normal path of this work. None of them makes this
condition impossible.

On an environmental fault (docker, gh, op, network, disk) run
`taskfmt selfhost env-fault "<reason>"` and continue. Never ask the operator.
```

The sentinel strings are exact and carry no extra fields; anything else means the goal never fires.

**Compaction**, three layers, weakest last: `step`'s stdout re-emits the invariants every turn (load-bearing, depends on nothing unverified); root `AGENTS.md` carries a short permanent block — edit `AGENTS.md`, **never write through the `CLAUDE.md` symlink**, since replacing it with a regular file fails `taskfmt selftest` repo-wide (`selftest.rs:548-551`) and so fails every meta-task's `[regression]`; and a `SessionStart:compact` hook emitting `taskfmt selfhost status`. Set `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE=50`.

**The supervisor** runs outside the session, because a session cannot relaunch itself. It owns the `env-fault` probe loop, the backoff and the resume. Three requirements:

- **The heartbeat comes from `taskfmt selfhost record --heartbeat`**, called by the dispatcher on each status poll — not from the `Stop` hook. A single dispatch turn legitimately runs for hours and emits no turn boundary.
- **Staleness threshold = `kill_after_min + 60` minutes** (i.e. ≥240 with item 7's value).
- **An `flock` lease on the ledger**, taken by `supervise` and by `step`. Without it the supervisor relaunches during a healthy dispatch and two orchestrators share one ledger and one repo, both dispatching the same cursor and both promoting to `main`.

`BASH_MAX_TIMEOUT_MS = 3600000` (1 hour). Long commands use `run_in_background` and poll.

### 4.5 Terminals

Three, and every one is reachable:

| Terminal | Condition |
|---|---|
| `SELFHOST-CHAIN-VERIFIED` | all seven tasks verified per §3.4 |
| `SELFHOST-NOT-PROVEN … reason=model-capability-limit` | §7.1 |
| `SELFHOST-NOT-PROVEN … reason=cycle-budget-exhausted` | **the global bound**: 6 green-field cycles, or 120 machine-hours, or a configured dollar ceiling — whichever first |

The budget terminal is not optional. Without it, `package` and `harness` faults have no cap: each repair green-fields, re-pays the whole chain, and resets every counter, so `repair → 3 reseeds → repair → 3 reseeds` cycles forever. With 13 recorded defects and nothing past TASK-003 ever dispatched, that is the likely path, not the unlucky one.

---

## 5. Verification — all phases

### 5.1 What a verifier is, and what independence means here

**A host Opus subagent**, per the operator's brief. Not a container: a Claude Code subagent cannot run inside one, and every other section of this document treats it as a host process. v3 said "container" in one section and "host" in four; this is the resolution.

It is given, and given only:

- `repo_url` (or a local bare mirror path), `pushed_sha`, `base_sha`
- a **copy of the task package's `README.md`, `decisions.md` and `verify.toml`** into its scratch directory

That second input is mandatory and v3 omitted it: the `AC-*`, `R-*` and `D-*` the verifier must check live in `experiments/tasks/TASK-00N/` in *this* repo and never reach the pgtui clone. Without them the verifier has no spec.

**Credentials.** The experiment repo is created `--private` (`gh.rs:8-16`), so a credential-free verifier cannot clone it. The driver (Phase 2) or the operator (Phase 1) fetches the SHA into a **bare mirror on the host** and points the verifier at that. Do not hand the verifier `gh`.

**Independence is enforced by convention plus a `PreToolUse` Bash deny** covering the runs directory, the repository root, the ledger path, `gh` and `op` — not by a mount. A host subagent with `Bash` can read anything the operator can; saying "structural" would be false. §0 records this as a silent-failure mode of the claim.

**Diff base** is the run's `base_sha` (the trusted overlay commit `planner: <TASK> trusted material`, `run.rs:145`), not the previous verified SHA — the pushed range contains that harness-authored commit, and diffing against the predecessor scores planner-supplied tests as out-of-scope agent work.

Model-family correlation is closed on the experiment substrate (Flash implements, Opus verifies) and open on the meta substrate (Opus both sides).

### 5.2 What the verifier checks

The host gate is necessary and not sufficient: the recorded TASK-003 run passed **11/11** gate checks while overriding fixed decision D-032, and wrote so in plain text in its handoff.

1. Clone fresh at the recorded SHA from the mirror; refuse if the ledger marks it `orphaned`.
2. Run every `AC-*` evidence command; capture output.
   **The gate AC is a special case.** Every package's last checklist leaf is the `taskfmt verify` gate, and on a fresh clone there is no `progress.md` (never committed) and no `baseline` tag (never pushed). Under a blanket "a command that cannot run is FAIL", a verifier PASS would be unreachable on every task forever. The verifier runs `taskfmt verify --base <base_sha> --task-dir <pkg> --no-progress` — all three flags exist (`cli.rs:71-92`) — and reports the progress check as covered by the host gate, not by itself.
3. Confirm every `R-* (MUST)` and `(MUST NOT)` against the diff from `base_sha`.
4. **Decision conformance** — every `D-*` the task's scope touches. The gate's blind spot; the verifier's main job.
5. Confirm the artifact works: build, run the real entry points, inspect gallery output where produced.
6. Write a **schema-validated verdict file**: `overall ∈ {PASS, FAIL, ABORT}`, per-AC array, per-R array, decision-conformance list, and on FAIL the single most decisive evidence.

**Mechanical checks belong to the driver, not the verifier**: deleted or weakened tests, added `#[ignore]`, added lint suppressions and out-of-scope changes are greps. Note `#[ignore]` is already a `[[forbidden_patterns]]` entry in the packages, so the gate catches it first — which is why §5.3's sabotage fixture uses a *deleted trusted test* as its primary signal and the `#[ignore]` case only as a secondary.

### 5.3 Calibration

A reviewer that passes a tree known to be defective has zero discriminative power. Before any contract's verdict is trusted it must **FAIL** both, and the results go in the ledger:

1. **`selfhost/fixtures/known-bad/task-003-decision-override/`** — the TASK-003 tree that passed 11/11 gate checks while violating D-032. Copy it out of `experiments/runs/` and **commit it**: it is gitignored, 7.9 GB in place, is exactly what `gc` reaps, and moves when `runs_dir` relocates. Commit only the source tree, not `target/`. **Pin it to the pre-amendment package hash** — once the D-032 amendment lands the tree fails for a mechanical reason instead of the judgement reason it was chosen for, and would silently stop testing decision conformance while still reporting "calibrated".
2. **`selfhost/fixtures/known-bad/deleted-trusted-test/`** — a passing tree with one trusted test removed.

**Calibrate once. The contract prompt templates are fenced** (§7.1), so "recalibrate whenever a template changes" is not a recurring cost.

---

## 6. The Phase 2 loop

### 6.1 Every outcome

| Outcome | Handling |
|---|---|
| dispatch returns `Err` before a gate exists | `DISPATCH_ERROR` → §7. The dispatcher returns `{outcome, error_class, run_dir?}` |
| gate PASS but status not promotable (`AGENT_EXITED`, `CONTAINER_STOPPED`, `KILLED_TIMEOUT`) | not a pass. Condition is `is_promotable(status) && gate.passed()` |
| `KILLED_TIMEOUT` | `docker stop` **before** gating — the kill path sends only `/goal clear` (`status.rs:127-130`, `:252-253`) and leaves the agent writing into the bind mount the gate is about to read |
| promote refuses or the push is rejected | `PROMOTE_ERROR` → §7. Never an agent-work defect |
| verifier crashes, times out, or returns an unparseable verdict | `ABORT`, not FAIL. Re-spawn once with identical inputs — the one legitimate no-distinguisher retry. A second `ABORT` is an environment fault |
| gate exit 70 (internal) | recorded like any other failure, never propagated before the record is written |

### 6.2 The cycle

```
0. RESUME: derive cursor from the ledger; never act on a remembered cursor
1. ensure repo; take the flock lease
2. PROBE: docker info · gh auth status · op whoami · gh api rate_limit · disk headroom
3. CRASH-WINDOW CHECK: does origin/main equal verified[cursor-1]?
      cursor==1: origin/main must be the bootstrap commit
      mismatch:  the previous attempt pushed without recording. Adopt the pushed SHA
                 only if a verifier verdict file for it exists and validates; otherwise
                 green-field the cycle. Never dispatch onto an unexplained main.
4. DISPATCH cursor (experiment --tasks <ID> --resume <cycle>, or run --task … --repo …);
   record the intent BEFORE any push
5. gate + status; if not (promotable && pass) → §7
6. PROMOTE; if refused → §7
7. VERIFY: fresh Opus verifier against the pushed SHA (§5)
8. if verdict FAIL/ABORT → §7
9. record verified[cursor]; gc; cursor = next
10. cursor past TASK-007 → SELFHOST-CHAIN-VERIFIED
```

Step 3 is the crash-window guard, and it is a SHA comparison rather than v3's selfcheck polarity probe — which returned NOVERDICT on TASK-001 (whose focused commands are unrunnable in an empty repo), ran the fixture toolchain on the credentialed host, and was redundant with this comparison anyway.

Without step 3, a crash between push and ledger-write leaves work on `main` with no record; the replay's fresh clone already contains the finished work, `base_sha == clone_sha`, the focused suites are green at base, the agent does nothing, and **the gate passes on a no-op run** — fabricating a data point.

---

## 7. Faults

| Class | Meaning | Response |
|---|---|---|
| **agent-work** | package and harness sound; the model did it wrong | replay with a `reseed` distinguisher, capped at 3 **per task per cycle**, then → §7.1 |
| **package** | the package is defective | fenced repair: planner authors and commits, blind-reviewer authorizes a token, implementer fixes, meta-verifier PASSes |
| **harness** | a `taskfmt` bug, a gate hole, a container problem | same path — `harness/**` is fenced, so a harness fix needs a token too. v3 said "no token" here and "token required" in the fence; the fence wins |
| **environment** | docker, gh, op, network, disk, rate limit, model unavailable | **never a code fix.** `env-fault`, then the supervisor's probe loop |
| **indeterminate** | cannot be established | a second diagnostician with a different framing, before any change |

### 7.1 `MODEL_CAPABILITY_LIMIT`

If GLM-5.3-Flash at effort `low` simply cannot do TASK-005, three reseeds produce identical `agent-work` diagnoses, no meta-task can touch "the model did it wrong", and changing the model is forbidden. If the only terminal is success, the sole remaining move that changes the distinguisher tuple is a fenced edit to the task — so the loop reclassifies `agent-work` as `package` and weakens the research object. **A goal whose only terminal is PASS will be reached by weakening.**

So after 3 reseeds with `agent-work` diagnoses:

```
SELFHOST-NOT-PROVEN model=zai-flash failed_at=TASK-005 reason=model-capability-limit
```

Terminal, satisfies the goal, and is a legitimate finding. The reseed counter is **per task per cycle and is not reset by a landed repair** — otherwise repair-then-reseed cycles indefinitely.

### 7.2 The repair token

`AUTHORIZED-REPAIR: <path>` as a line of prose is a string the loop types itself. Instead: `taskfmt selfhost token mint --path <p> --diagnosis <id> --reviewer-verdict <file>` emits an HMAC over `(fenced path, diagnosis id, blind-reviewer verdict SHA-256, cycle)`, keyed by a file the operator creates at pre-flight and no agent can read. `token verify` checks it; `taskfmt selftest` calls `token verify`, not a string match. The blind-reviewer (§4.3) is never shown the failing run's diff.

**The `[agents]` profile block takes no token.** It is an unconditional deny in `diff-check` and in `token mint`, not a WEAKENING class that a token could authorize.

### 7.3 Anti-spin

The distinguisher is machine-derived: `(taskfmt_binary_sha256, image_digests, harness_sha, package_tree_hash, reseed_counter)`, all recorded per attempt. Two attempts with the same tuple are forbidden regardless of what the record claims — which closes the case where a fix was compiled but never installed, or the images were never rebuilt.

Three consecutive attempts in the same fault class suspend dispatch until a meta-task lands whose fix touches a file in that class; dispatch resumes on that landing. For `agent-work`, §7.1 is the exit.

### 7.4 Revoking a verdict

The diagnostician emits `earliest_bad_task = k`. **§8's replay table decides the scope; this rule decides only the cursor.** If the fix lands in a green-field row, the cycle green-fields and `verified` is discarded entirely — a chain assembled across two research objects is not a chain. Otherwise the driver drops `verified[k..]`, rewinds to `k`, and (Phase 2) resets to `verified[k-1]`.

---

## 8. The fence and replay scope

### 8.1 What is fenced

```
experiments/tasks/**            experiments/references/**
reference/task-template/**      harness/goal-prompt.md
experiment.toml [agents] block  AGENTS.md (root)
.claude/agents/**               selfhost/fixtures/**
harness/** except harness/target/, harness/src/cmds/selfhost.rs, harness/src/selfhost/**
```

**By exclusion, not enumeration** — v3's enumeration omitted `runstate.rs`, the code that writes the gate verdict. The driver is carved out **by path** so its own bugfixes do not trip the fence; v3 wrote "the ledger types in `runstate.rs`", which is not a path, so move those types to `harness/src/selfhost/ledger.rs`.

**The fence engages when Phase 2 starts.** Phase 1 pre-flight edits `experiment.toml`, root `AGENTS.md` and the harness freely; that is operator work under review, not loop work.

### 8.2 Rules

- Editing a fenced path requires a **token** (§7.2) and costs the replay scope in §8.3. Cheap edits are confined to `selfhost/**`, `docs/**` and tests that only add coverage.
- **`experiment.toml`'s `[agents]` block is immutable.** Swapping `zai-flash` for a stronger model, or raising effort, is the cheapest way to turn a red chain green and proves nothing. Unconditional deny; §7.1 exists so this cannot deadlock the loop.
- **Fence-set entries and `expected_paths` entries under `experiments/` or `reference/` are literal paths** — no `*` or `?`. `*` crosses `/` in the gate's glob dialect (`gate.rs:391`), so `experiments/tasks/*` would cover all seven packages including their `trusted/` oracles.

### 8.3 Replay scope

**The question is: did the change alter what the gate judges, what the agent is told, or what the agent is judged against?**

| Change | Replay |
|---|---|
| `gate.rs`, `progress.rs`, `selfcheck.rs`, `lint.rs`, `taskfile.rs`, `verifycfg.rs`, `cmds/verify.rs`, `cmds/gate.rs`, `cmds/progress_init.rs` | **green-field** — the judge changed |
| `experiments/tasks/**`, `experiments/references/**`, `reference/task-template/**`, `harness/goal-prompt.md`, `experiment.toml`, `cmds/agent_launch.rs`, `cmds/container_entrypoint.rs` | **green-field** — the contract or the protocol changed |
| `harness/images/**` | **green-field** — the baked-in gate changed |
| `runstate.rs`, `cmds/promote.rs` | **green-field** — the record changed |
| `ops/git.rs` **scope paths** (`changed_files` and its callers) | **green-field** — the scope check is part of the judge |
| plumbing: `cmds/run.rs` timeouts, `cmds/status.rs` detection, `cmds/repo.rs`, `interactive.rs`, `ops/docker.rs`, `ops/gh.rs`, `ops/git.rs` non-scope paths, `config.rs`, `cli.rs`, `redact.rs`, `selection.rs`, `selftest.rs`, `cmds/experiment.rs`, `Cargo.lock`, `harness/tests/**` | **failing task only** |
| `harness/src/cmds/selfhost.rs`, `harness/src/selfhost/**`, `selfhost/**`, `docs/**`, `.claude/**` | **failing task only** |
| tests that only add coverage | **failing task only** |
| **anything not listed** | **green-field** — deny by default |

**Precedence is scoped within the table**: among matching rows the most invalidating wins, but a change **confined** to a "failing task only" path takes that row. v3's unqualified "most invalidating wins" deleted its own exceptions.

For `cursor == TASK-001`, always green-field — the operator's rule.

**Standing obligation.** Every fix touching `harness/**` ends with `cargo build --release && install && taskfmt preload && taskfmt build-images`. The gate is baked into the images (`images/base/Dockerfile` `COPY --from=harness-taskfmt:latest`); without a rebuild the host gate runs the new engine and the container runs the old one, and the agent's two-phase endgame converges on the wrong oracle. The binary hash and image digests are the distinguisher (§7.3).

### 8.4 `diff-check` — Phase 3

A **sound over-approximation**, not a classifier. Only a short allowlist of provably-additive patterns is STRENGTHENING: a new `[focused]`/`[regression]`/`[lint]` command, a new `[[forbidden_patterns]]`, a new `R-*`, a new file under `trusted/` or `experiments/references/`. **Every other diff to a fenced path is WEAKENING and needs a token**; the seven decidable classes (editing under `trusted/` · dropping `-D warnings` · changing `kind:` · changing the task-id set · removing a gate command · removing a forbidden pattern · deleting an `R-*`) emit a specific reason, everything else emits "unclassified change to a fenced path". ~300 lines.

This is over-approximate because the cases that matter are undecidable: rewriting an `R-*` to require less; rewriting a `D-*` to match what an agent did (the classifier takes two trees and never sees the agent's diff); and editing `reference/task-template/AGENTS.md` "in a way that adds executor guidance" — the most consequential fenced file, since it is the protocol injected into every run, and the least mechanizable. Glob widening is regex-language containment with no normative dialect statement. "Narrowing a test command" is half-built (`lint::CargoTestSpec::narrower_than`, `lint.rs:920`) but `parse_cargo_test` returns `None` on compound commands — and three of TASK-001's six focused commands are compound `grep -qE … && grep -qE …` pin checks, i.e. exactly the ones worth weakening.

---

## 9. The meta-task substrate — Phase 2

`selfhost/AGENTS.md` is the host delta from the template protocol, with a sibling `CLAUDE.md` symlink.

| Template clause | Host replacement |
|---|---|
| `/task` ro, `/work` rw, `/progress` rw | the package dir, the `main` checkout, `$XDG_STATE_HOME/taskfmt-selfhost/runs/<cycle>/TASK-1NN/progress.md` |
| `/task` read-only *by mount* | no mount exists. **The package lists its own directory in `forbidden_paths`**, so any edit to it fails the gate — which is why the planner commits it *before* `TASKFMT_BASE` is captured (§4.3) |
| gate is the baked-in binary | `cargo run -q --manifest-path harness/Cargo.toml -- verify`, never the `PATH` binary. The meta-verifier's `selftest` runs the same way |
| `$TASKFMT_BASE` | the spawner exports `TASKFMT_ROOT`, `TASKFMT_TASK_DIR`, `PROGRESS_FILE`, `TASKFMT_BASE` (the tip **after** the package commit; never a SHA in `base_ref`, which goes stale and lints clean) and runs `taskfmt progress-init <pkg> -o "$PROGRESS_FILE"`. **The spawner is `taskfmt selfhost step`** |
| — | **Never `git stash`, never move `TASKFMT_BASE`.** (v3 said "never `git commit`" on the grounds that a commit empties the base diff — false: the gate diffs base against the *working tree*, `git.rs:177-197`. `stash` and re-pointing the base are the real hazards.) The implementer commits to `main`; the orchestrator pushes after the gate |
| — | **New-test rule:** a focused command must name a **new `--test` target file**. A new filter inside an existing target exits 0 at base ("0 filtered out") — a silently green baseline. Only a missing target is genuinely red |
| — | Commands use `--manifest-path harness/Cargo.toml`; the workspace is not at the repo root, and `cd harness && …` defeats the linter's `cargo test` matching (`parse_cargo_test` requires token 0 to be `cargo`) |
| `NEEDS_REPLAN` triggers | add: the fix requires editing a fenced path and no valid token exists |
| protocol steps 1–8, progress grammar, prohibitions, stop conditions, turn signal, final report | **kept as close to verbatim as the substrate allows.** Steps 1, 2 and 8 necessarily rewrite their paths and gate command; the rest is unchanged, and the identical transcript grammar is what makes the substrates comparable |

**One implementer at a time, in the `main` checkout.** The repository works on `main` only (root `AGENTS.md`), so meta-tasks lack the fresh-clone isolation experiment tasks have — a stated exception, and part of why §0 says the meta substrate is not evidence about the research question. Serialization is enforced by the orchestrator's one-subagent-per-turn rule. The out-of-tree ledger and gitignored `selfhost/reports/` keep orchestrator artifacts out of the gate's changed set.

**Selfcheck before the implementer.** `taskfmt selfcheck <pkg> <workspace> --base <sha>` — **both arguments are positional**; there is no `--workspace` flag (`cli.rs` `Selfcheck { task, workspace, --base, --reference, --keep }`), and v3's command as written would not parse. It must PASS before an implementer is spawned; otherwise nothing enforces a red baseline, since selfcheck is opt-in at dispatch and the `baseline` lint rule is a WARN whose suite comparison is restricted to `kind: feature|bugfix`.

**Exception:** for a meta-task whose `expected_paths` includes the gate engine itself, the pre-implementer selfcheck is **skipped and recorded as skipped** — the pre-fix binary is by definition the broken one, so requiring it to pass makes the fix ungateable. The meta-verifier compensates by confirming `harness/tests/gate_tamper_matrix.rs` is unchanged and that the new tests fail against the pre-fix binary.

**Fence enforcement, three mechanical layers:**

1. `forbidden_paths` per package — exact-prefix on the changed set (`gate.rs:422-441`), zero cost when untouched.
2. `allowed_globs == expected_paths`, already lint-enforced (`lint.rs:1306-1330`), so an unlisted fenced path fails `scope` independently.
3. `taskfmt selftest` gains three repo-wide checks: every `selfhost/tasks/*/README.md` carries the disclaimer verbatim; its `verify.toml` forbids fenced prefixes unless `token verify` passes; and **`taskfmt lint selfhost/tasks/*` runs**, since `paths.tasks_dir` is a single value and bare `taskfmt lint` never sees the meta corpus. These three are a Phase 2 work item, TASK-108.

Every `selfhost/tasks/*/README.md` carries verbatim under `## Goal`:

> This is a meta-task. It is scaffolding for proving the project, not part of the research object. `experiments/tasks/**` is the source of truth; this package is not.

---

## 10. Work items

**Phase 1 pre-flight** — operator-authored: §3.1 items 1–9, including TASK-101, TASK-105, TASK-106 as ordinary operator commits (the fence is not yet engaged).

**Phase 2 pre-flight** — operator-authored, before the session: the driver (§4.1 Phase 2 subset), the eight `.claude/agents/*.md` definitions, `.claude/settings.json`, `selfhost/AGENTS.md` + symlink, the root `AGENTS.md` block, the token key, TASK-108's selftest checks, and the smoke corpus (a trivial `TASK-9xx` — write a file, one focused grep — so the first end-to-end dispatch test is minutes, not hours). **The orchestrator must never be its own bootstrap.** About 3–4 weeks.

**Phase 2 loop-executed:**

| ID | Goal |
|---|---|
| **TASK-102** | driver extensions beyond the Phase 2 subset |
| **TASK-103** | reference solution for TASK-002 under `experiments/references/`, plus a committed base workspace fixture; the **oracle phase runs green for the first time**. ~1 week — it is hand-writing a working pgtui increment |
| **TASK-104** | container-mode selfcheck, so TASK-001 stops returning NOVERDICT |
| **TASK-107** | host/image version check — `run` refuses on mismatch (backlog 25). **Phase 3 prerequisite** |
| **TASK-109** | the D-032 amendment (§10.1) |

### 10.1 TASK-109 — the D-032 amendment

`experiments/tasks/TASK-003/decisions.md:110` (D-032) requires all fields non-empty except Password; `trusted/…/screen_create_form_test.rs:62-71` types only a name, presses Enter, and asserts `SaveConnection` is `effects[0]`. Unsatisfiable in both directions — the passing run's `validate()` checks name only, and the failing run implemented D-032 faithfully and panicked at line 69.

- **It is scheduled**, as TASK-109, which v3 failed to do while calling it critical-path.
- **It green-fields the cycle** (§8.3 row 2), so it runs at the start of a cycle, never mid-chain.
- **It goes through the normal fenced path**: planner authors, blind-reviewer authorizes a token for `experiments/tasks/TASK-003/trusted/crates/pgtui/tests/screen_create_form_test.rs`, meta-verifier PASSes. v3 exempted it as a "decision record", making the single most sensitive edit in the plan the only one with no fence on it.
- **Polarity is fixed in advance: the `D-*` is authoritative and the trusted test is corrected to match.** Otherwise the project retro-fits its spec to a run's output — which is exactly what the one passing run did.
- **Acceptance is a conformance judgement**, not a pass rate: a fresh verifier, given only D-032 and the amended test, agrees the test now exercises the decision. v3's "accepted on two consecutive green runs" rewards whichever version the agent passes more often, and the correct amendment is the *stricter* one — a gradient toward the weaker spec, written into the place the plan says to watch hardest.

---

## 11. Operator preconditions

Docker running with **≥600 GB** allocated (Phase 3's ceiling; Phase 1 needs ~100 GB) · `gh auth refresh -s delete_repo` · `OP_SERVICE_ACCOUNT_TOKEN` exported and headless `op read` proven · repo clean on `main` · workspace trusted · `caffeinate -dimsu` around any long run.

Phase 2 launch: `claude --agent selfhost-orchestrator --permission-mode bypassPermissions`, then paste §4.4. Start `taskfmt selfhost supervise` first.

---

## 12. Costs

From the manifests (`Manifest.start` → `gate.finished`):

| Run | Duration | Gate |
|---|---|---|
| TASK-001 ×2 | 2.6, 3.8 min | pass, pass |
| TASK-002 ×2 | 16.3, 16.3 min | fail, pass |
| TASK-003 ×2 | 34.7, 14.3 min | pass, fail |

The 34.7-minute run was gated manually, well after the agent went quiet; the loop's own TASK-003 took 14.3 minutes. Harness completion-detection latency is bounded by a 90 s warmup plus a 30 s settle against a 300 s activity window (`status.rs:212`), so it cannot exceed ~5.5 minutes — v3 attributed 24 minutes of operator wall-clock to the harness and would have sent someone chasing a bug that does not exist.

Modelled with corpus growth (cumulative trusted tests 4 → 20 → 35 → 48 → 73 → 90 → 102; testcontainers from TASK-004): 5/20/35/50/70/80/100 min ⇒ **≈6 h container time per chain**, serial.

Verifiers: each is a cold Rust build of the full dependency set plus the cumulative suite; 20–30 min for TASK-001..003 and 45–90 min for TASK-004..007, serialized ⇒ **4.0–7.5 h per chain**.

**Per chain: 10–14 machine-hours**, plus 7 Opus verifiers at $5–20 each.

| | Machine-hours | Dollars | Disk |
|---|---|---|---|
| **Phase 1** (1 chain + verifiers, plus reruns) | 15–30 | $100–400 | ~100 GB |
| **Phase 2** (loop, repairs, green-fields; budget-capped at 120 h) | 40–120 | $500–2,000 | ~250 GB |
| **Phase 3** (measure p, freeze, cheat seed ×3, k=2) | 35–55 | $500–1,500 | 350–600 GB |

`gc --keep-last 2` is mandatory from the first chain: `experiments/runs/` is **33 GB for seven runs** on this host, TASK-003 alone being 7.9 GB.

---

## 13. First five sessions

1. **Make the host able to run anything (½–1 day).** Docker with ≥300 GB. `cargo install --path harness`; `taskfmt preload`; `taskfmt build-images --agent claude` — **expect it to fail the first time**. `gh auth refresh -s delete_repo`; 1Password service account; prove headless `op read`. *Exit:* `taskfmt run --task experiments/tasks/TASK-001 --repo <fresh>` reaches a live agent pane and `taskfmt attach` shows it.
2. **Pre-flight items 2, 5, 6, 7 (1–2 days).** Item 6 (TASK-101) before session 3, because that is what makes `--auto` safe on a mutating path. Then `cargo fmt --check && cargo clippy -D warnings && cargo test && taskfmt selftest`.
3. **One attended chain, watched (1–2 days, mostly waiting).** `taskfmt experiment --tasks all --auto` with **no** `--repo` and **no** prior `taskfmt repo create` — `experiment` mints and records the repo itself, and creating one first just orphans it. Build no orchestrator. The point is to discover what breaks at TASK-004..007: **nobody knows, because nothing past TASK-003 has ever been dispatched**, and TASK-004 is where Docker-in-Docker and testcontainers enter. Expect to stop, fix, and green-field at least once.
4. **The verifier contract and its calibration (1 day).** Write `.claude/agents/verifier.md`. Copy the known-bad TASK-003 tree into `selfhost/fixtures/` **first** — it is gitignored and `gc` will reap it. Then calibrate against both fixtures. If the verifier passes either, everything downstream is theatre — find that out in day four, not week six.
5. **Verify the chain; write the honest sentence (1 day).** Run the calibrated verifier against each pushed SHA from session 3, serialized, against a local bare mirror, with the task package copied into its scratch dir. Append verdict files and their hashes to the out-of-tree ledger. Then write down which of §1's claims the evidence supports.

That completes **Phase 1**. Only then decide whether Phase 2's three-to-four weeks of orchestration engineering, and Phase 3's freeze, are worth it.

---

## 14. Deferred

Reference solutions beyond TASK-002; research metrics beyond the six recorded per attempt; the Codex substrate (the `codex-default` profile has no `env_secret`, and `taskfmt codex-login` is invoked by the container entrypoint at `container_entrypoint.rs:71` and implemented at `:336` but has no `cli.rs` variant, so it cannot be called); the format ablations in the research backlog; and the recorded harness defects that block no chain.

## 15. Known deviations from the operator's brief

1. **The orchestrator's vote is non-binding** (Phase 3 consensus). It delegated everything and its goal terminates on PASS. It attests that the process ran; the attestation is recorded.
2. **"Never be blocked"** is re-scoped to "never asks, never spins, auto-resumes". A session cannot repair its own auth, credits or disk. Pre-flight item 4 removes the most likely cause outright.
3. **"Authorized to change everything"** is narrowed by the fence, with one absolute: the `[agents]` profile block. §7.1 exists so this cannot deadlock the loop.
4. **A cross-family consensus reviewer** (Phase 3 only) would contradict "subagents must be Opus"; it is the only thing that makes agreement mean more than correlation. Needs sign-off, or drop it.
5. **`experiments/references/`** is new content inside `experiments/`, though outside `experiments/tasks/**`. It is fenced.
6. **The meta-task protocol is a fork** of the template's `AGENTS.md`; the container assumptions have no host equivalent.
7. **Phase 1 is operator-attended.** The brief asks for an unattended session; that is Phase 2. Phase 1 exists because the fastest honest path to a verified chain does not require the orchestration layer, and because nothing past TASK-003 has ever run.
8. **The largest single work item — the driver — is operator pre-work**, not delegated to the session, because the orchestrator cannot bootstrap the state store it depends on while modifying the harness underneath it.
