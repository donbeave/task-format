# task-format — How It Works

**Purpose of this document.** A single, evidence-backed description of what this project is, how every layer is supposed to work, what is actually verified today, and where the design and the code disagree. Written from a full read of `docs/research/RESEARCH-FINDINGS.md`, `reference/task-template/`, the whole `harness/` crate and test suite, `experiments/tasks/TASK-001..007`, and the one recorded run in `experiments/runs/`.

**Source-of-truth relationship.** `docs/research/RESEARCH-FINDINGS.md` remains the normative record of *decisions and evidence*. This document is the *operational map*: what the parts are, how they fit, and how to run them. Where the two disagree, the findings file wins on rationale and this file wins on current code behavior — every behavioral claim here is cited to a file.

**Status in one line.** The tooling is built, tested and green (191 tests, clippy clean, `taskfmt selftest` 31/31); the *research* has produced **zero measured experimental runs**, so every predictability claim is still a hypothesis.

---

## 1. The research question

> Which markdown task-package structure gives the most predictable output from **one** coding agent handed **one** bounded task in a **fresh** container on a **fresh** clone?

"Predictable" is defined operationally, not aesthetically: **low variance across seeds, plus a deterministic outer gate that can decide pass/fail without consulting the agent**. The explicit tie-breaker (`RESEARCH-FINDINGS.md:177`):

> A variant that raises `gate_pass` but raises `false_done` or scope variance is *not* more predictable.

The failure mode being attacked (`RESEARCH-FINDINGS.md:11`): long `/goal` runs drift when they bundle several independently-completable outcomes, unresolved design decisions, unrelated repo context, repeated compaction, subjective completion criteria, and self-verification by the implementer.

The architecture the whole project assumes (unchanged since the first research note):

```
big goal  →  planned task DAG  →  one ready task  →  one fresh context
          →  one isolated workspace  →  deterministic verification
          →  independent completion decision
```

**The DAG and the planner are out of scope.** This project studies the *single-task package* only (`RESEARCH-FINDINGS.md:17`).

### What varies, what is held constant, what is measured

- **Independent variable:** the task-package structure. Because protocol was split out of task content (D2), `README.md` (task content) and `AGENTS.md` (protocol) are *separately* manipulable — you can hold one fixed and vary the other. That separability is the reason the split exists.
- **Held constant per variant:** base commit, model, effort, seed, verifier, container, network policy, budget, turn cap. Pinned in code by `experiment.toml` profiles + the recorded `manifest.json`.
- **Measured, always harness-computed and never taken from agent claims** (`RESEARCH-FINDINGS.md:175`): `gate_pass`, `false_done`, `false_blocked`, `task_tamper`, `scope_violation`, `leaf_claim_accuracy`, `state_consistency`, `report_conformance`, `ac_coverage`, `verify_runs`, turns/tokens/cost/wall/compactions, `diff_stability`, `instruction_violations`, `changes_before_first_ac1_pass`, `false_done_vendor`, `cheat_seed_gate_pass`, `incomplete_rate`.
- **Minimum design:** ≥3 task kinds × ≥5 seeds × variant, reported as mean + stddev.

---

## 2. The mental model — five layers

Everything in the repo belongs to exactly one of these. Keeping them straight is most of the "how it works".

| Layer | Artifact | Who owns it | Who may write it |
|---|---|---|---|
| **1. Task package** | `README.md`, `verify.toml`, `decisions.md`, `trusted/` | planner (human) | nobody at run time — mounted read-only |
| **2. Agent protocol** | `AGENTS.md` (+ `CLAUDE.md` symlink) | fixed template | nobody |
| **3. Gate** | `taskfmt verify` binary + `verify.toml` data | harness | nobody — baked into the image |
| **4. Execution harness** | container, herdr, mounts, secrets, dispatch | harness | — |
| **5. Orchestration** | experiment loop, repo lifecycle, promote | operator + harness | — |

The one agent-writable file in the entire system is **`/progress/progress.md`**. That is the design's central bet: *the agent's only channel for state is a file whose grammar the gate checks byte-for-byte against a contract it cannot edit.*

Repo layout mapping to layers:

```text
reference/task-template/   layers 1+2 — the pure template, the research object itself
harness/                   layers 3+4+5 — the Rust CLI `taskfmt`, images, tests
experiments/tasks/         layer 1 instances — TASK-001..007 (the corpus under test)
experiments/runs/          run outputs (gitignored)
experiment.toml            the experiment manifest (schema experiment/v1)
docs/research/             the "why": findings, notes, raw inputs
```

---

## 3. Layer 1 — the task package (schema `task/v4`)

### 3.1 File inventory

A task package is **any directory containing `README.md`** (`harness/src/cmds/mod.rs:251`). Packages live under `experiment.toml`'s `paths.tasks_dir` = `experiments/tasks/`.

| File | Required | Mount | Writable by agent |
|---|---|---|---|
| `README.md` | **yes** | `/task` | no |
| `verify.toml` | lint warns if absent; **gate hard-fails** (exit 2) | `/task` | no |
| `AGENTS.md` | injected at dispatch from the template if absent | `/task` | no |
| `CLAUDE.md` | relative symlink → `AGENTS.md`, created at dispatch | `/task` | no |
| `decisions.md` | optional; **binding when present** | `/task` | no |
| `trusted/` | optional; overlaid into `/work` and committed *before* the agent starts | `/work` | no (outside every whitelist) |
| `progress.md` | **never in the package** — generated per run | `/progress` | **yes — the only one** |

`trusted/` is the SWE-bench idea adapted: planner-owned tests and support material land on the run's base commit, so the agent can neither see them as "its" tests nor modify them without failing scope (`harness/src/cmds/run.rs:135-150`).

### 3.2 `README.md` — the contract

**Frontmatter** (hand-parsed, not YAML; `harness/src/taskfile.rs:111-177`):

| Key | Rule |
|---|---|
| `schema` | must be exactly `task/v4` |
| `id` | `^TASK-[0-9]+$` |
| `title` | non-empty |
| `kind` | one of `bugfix feature refactor removal migration test docs` |
| `verify` | non-empty; canonically `taskfmt verify` |
| `expected_paths` | non-empty block list; **must equal `allowed_globs` in `verify.toml`** |

**H1** must start `# <id> — `.

**Eight required H2 sections, in this exact order** (`harness/src/lint.rs:231-271`; extra H2s allowed, but these eight keep their relative order):

1. `## Goal` — one sentence: a state that must become true.
2. `## Context` — conventionally four blocks: `Current behavior:`, `Desired behavior:`, `Read before editing (orientation only, non-normative, in order; never /task/*):` (a numbered list — **may not reference `/task/`**, lint rule C8), and `Baseline (run from repo root, before any edit):` with a fenced command plus `Expected before this change:`.
3. `## Preconditions` — `- **P-001:** <state> — \`<command exiting 0>\``. A precondition without a command is not a precondition, it is context (D10).
4. `## Scope` — `In scope:` / `Out of scope:`.
5. `## Requirements` — `- **R-001 (MUST):** …` / `(MUST NOT)`. Every `R-*` must be cited by an AC row or a checklist item (lint C3, ERROR).
6. `## Acceptance criteria` — one table: `| ID | Given / When / Then | Evidence command | Expected |`. Negations are written `! grep …` so the command still exits 0 on pass.
7. `## Fixed decisions` — `- **D-001:** …`, plus `Full text: /task/decisions.md (binding, read-only)` when a decisions file ships.
8. `## Checklist` — exactly one block between `<!-- checklist:start -->` and `<!-- checklist:end -->`.

### 3.3 Checklist grammar

```
^( {4}){0,3}- \[([ x])\] \*\*([0-9]+(?:\.[0-9]+){0,3})\*\* (.*)$
```

- Indent 0/4/8/12 spaces; `depth = indent / 4`; **ID component count must equal depth + 1**.
- IDs are contiguous: first is `1`; a child is `prev.1`; a sibling increments at its depth.
- The checkbox token `[ ]` / `[x]` is **the only three characters the agent may ever change**.
- Every leaf's text must contain `evidence:`, and the evidence must name a backticked command or claim "exits 0" (lint C1). The gate leaf is exempt.
- **The last leaf must be the `taskfmt verify` gate leaf** (D6).
- 5–20 leaves. No single-child parents. Max depth 4.
- Two items may not carry identical evidence text (lint C5) — this is what killed the per-AC proof leaves (D34; 24 duplicate pairs were found in the v3 corpus).
- Every `AC-*` must be cited (backticked) on a line that itself carries `evidence:` — a leaf, or a parent that has its own evidence.

### 3.4 `verify.toml` — declarative gate config (schema `verify/v1`)

Parsed with `deny_unknown_fields` on every struct (`harness/src/verifycfg.rs`). Misplaced top-level keys are **rejected, not silently dropped** — in TOML any key after a `[table]` header nests into that table, so all top-level keys must precede `[focused]`.

| Key | Meaning |
|---|---|
| `schema` | must be `"verify/v1"` |
| `base_ref` | scope base, lowest precedence |
| `allowed_globs` | **the scope whitelist**; must equal frontmatter `expected_paths` |
| `required_paths` | must exist in the worktree after the task |
| `forbidden_paths` | must be **absent from the changed set** vs base (existing is fine, modifying is not) |
| `[[forbidden_patterns]]` | `regex` + optional `paths`; run as `grep -rIEn`; any hit fails |
| `[focused].commands` | the task's own new tests — must be RED at base |
| `[regression].commands` | the cumulative suite |
| `[lint].commands` | fmt/clippy |

Glob dialect: only `*` and `?` are special, and **`*` crosses `/`**.

### 3.5 The linter — the format's real specification

`taskfmt lint` is the normative encoding of everything above. Output is `ERROR <rule>: …` / `WARN  <rule>: …`, then `SUMMARY errors=N warnings=M`, then `LINT PASS|FAIL`. **Exit 1 iff ≥1 ERROR; warnings never fail.**

Rules by name: `readme`, `frontmatter`, `sections`, `heading`, `placeholders`, `ids`, `context`, `preconditions`, `acceptance`, `checklist`, `requirements`, `baseline`, `commands`, `config`, `size`.

The nine named checks from D37:

| Code | Check | Severity |
|---|---|---|
| C1 | leaf evidence names a command or claims "exits 0" (gate leaf exempt) | ERROR |
| C2 | no duplicate `P-`/`R-`/`D-`/`AC-` IDs | ERROR |
| C3 | every `R-*` cited by an AC row or checklist item | ERROR |
| C4 | no template `<...>` placeholders left (set derived from the template itself — **no embedded fallback**, a missing template is a hard error) | ERROR |
| C5 | every AC cited on an evidence-bearing item; no two items share evidence text | ERROR |
| C6 | AC evidence command absent from `verify.toml` lists | WARN |
| C7 | `cargo test` with several positional filters and no ` -- ` | WARN |
| C8 | `/task/` referenced in the "Read before editing" hints | ERROR |
| C9 | baseline command ≠ AC-001 command or any gate command (`feature`/`bugfix` only) | WARN |

C6 and C9 compare `cargo test` invocations by **parsed spec equality** (package set, target set, filters, args after ` -- `) — order-insensitive, not string matching.

`taskfmt run` lints before dispatch and **aborts on any ERROR**; `progress-init` refuses to generate from a README with lint errors.

---

## 4. Layer 2 — the agent protocol (`AGENTS.md`)

Shared by every task, ~938 words. Split out of task content by D2 precisely so it can be varied independently.

### 4.1 What the agent sees

| Path | Mode | Role |
|---|---|---|
| `/task/README.md` | ro | the contract |
| `/task/decisions.md` | ro | binding when present |
| `/task/verify.toml` | ro | "never edit" |
| `/progress/progress.md` | **rw** | the only state file |
| `/work/` | rw | the repository |

The gate is `taskfmt verify`, baked into the image, read-only. `$TASKFMT_BASE` is the scope base commit.

### 4.2 The eight-step protocol

1. Read `README.md` fully (+ `decisions.md`), then the "Read before editing" files.
2. Non-empty `## Log` ⇒ **resuming**: read it, `git status`, `git diff --stat`, continue from `CURRENT:`. After compaction, re-read both files — *never reconstruct state from memory*.
3. State in the transcript: task ID, one-sentence goal, AC IDs, first leaf.
4. Run every precondition. Failure ⇒ `STATE: BLOCKED` + the failing command + `STATUS: BLOCKED`. **Do not work around a failed precondition.**
5. Work leaves one at a time in ID order; set `CURRENT: <id>` before starting.
6. Mark `[x]` only after running the evidence command and seeing the stated result; append one log line. Invalidation ⇒ back to `[ ]` + `REOPENED`. A check that both passes and fails on the same tree is FAILED evidence ⇒ stop `NEEDS_REPLAN`.
7. A parent goes `[x]` only when every child is `[x]`, and if it has its own `evidence:`, only after running and logging that too. **Parents are never `CURRENT`.**
8. **Two-phase endgame (D31)** — this is the fix for a protocol that was previously impossible:
   - **8a:** run `taskfmt verify --progress ""` from `/work` until exit 0 with last line `DONE`.
   - **8b:** set `STATE: DONE`, `CURRENT: NONE`, check the remaining leaves, run the *full* `taskfmt verify`, emit the final report. Only 8b's output is transcript evidence.

   (Without the split, step 8 was circular: the progress check requires `STATE: DONE` and every leaf `[x]`, which cannot be true while you are still iterating on the gate.)

### 4.3 `progress.md` grammar

```
---
TASK: <id>
STATE: IN_PROGRESS | DONE | BLOCKED | NEEDS_REPLAN
CURRENT: <leaf id> | NONE
BASELINE: <command> -> <observed result>
---

<!-- checklist:start -->
…verbatim copy of the README checklist, checkbox tokens only may differ…
<!-- checklist:end -->

## Log
- <id> | DONE|FAILED|REOPENED|BLOCKED | <command> -> <result>

## Handoff
CURRENT_FAILURE:
DECISIONS:
```

Rules: never change checklist text, IDs, order or indentation. Never change `TASK:`. `BASELINE:` must hold a real command and observed result before `DONE` — the gate rejects `<not run>`. **Missing work is not a new checklist line; it is `NEEDS_REPLAN`.**

`progress.md` is generated per run by `taskfmt progress-init` from `README.md` and is **never committed** (enforced by `.gitignore`).

### 4.4 Prohibitions and stop conditions

Prohibited: editing `/task/` or the `taskfmt` binary; deleting, skipping, weakening or rewriting a failing test or check; special-casing fixtures or verifier inputs; suppressing errors, warnings, lint rules, type checks or exit codes; changing any file outside `expected_paths`; changing user-visible behavior that no `R-*`/`AC-*` names (note it under `FOLLOW_UP`); claiming `DONE` without a `taskfmt verify` run in this session whose output is in the transcript.

Stop conditions:

- **`BLOCKED`** — environment/dependency only.
- **`NEEDS_REPLAN`** — goal/AC/decisions/scope/checklist would have to change; requirements contradict; an unresolved material design decision.
- **`INCOMPLETE`** (D32) — turn/budget cap. Leave `STATE: IN_PROGRESS`, fill the handoff. *"Reaching a budget limit is not the same as completing the objective."*
- *"Do not spin."*

### 4.5 Turn signal and final report

Every turn ends with `GOAL_PROGRESS task= state= current= done_this_turn= blocked=`. The run ends with `STATUS:`, `TASK:`, `SUMMARY:`, `ACCEPTANCE:` (one line per AC), `VERIFY: command=taskfmt verify exit=<n> last_line=<DONE|other>`, `CHANGED:` (**verbatim** `git diff --no-renames --name-status $TASKFMT_BASE` plus untracked lines — *not recall*, D38), `DEVIATIONS:`, `FOLLOW_UP:`, `GOAL_RESULT task=… status=…`.

---

## 5. Layer 3 — the gate

### 5.1 The pass definition

**Exit 0 AND last stdout line exactly `DONE`.** Nothing else is a pass. An internal error becomes `RESULT FAIL internal-error …` with exit 70 — never a silent `DONE`.

Exit codes: `0` pass · `1` a check failed · `2` `verify.toml` missing · `70` internal error.

### 5.2 Ordered checks

```
config → scope → required_paths → forbidden_paths → forbidden_patterns
      → focused.1..N → regression.1..N → lint.1..N → progress
```

Each writes `<log_dir>/<name>.log` and emits `CHECK <name> PASS` or `CHECK <name> FAIL rc=<rc> log=<path>` with a 40-line tail. Then `SUMMARY pass=N fail=M log_dir=…`, then `RESULT PASS` + `DONE`, or `RESULT FAIL`.

All commands run under `bash -eo pipefail -c` from the repo root (D33) — so `false; true` fails and `false | true` fails.

**Scope is fail-closed** (D33): `--no-renames`; untracked enumerated with `--exclude-per-directory=.gitignore` plus untracked `.gitignore` files listed explicitly; `skip-worktree`/`assume-unchanged` index entries fail the check outright; an empty `allowed_globs` fails.

**Base precedence:** `--base` > `TASKFMT_BASE` env > `base_ref` in `verify.toml` > the `baseline` tag. Dispatch passes the **recorded base SHA**, not the movable tag, because the tag lives in agent-writable `/work`.

### 5.3 The `progress` check

In order: file exists; README exists; **checklist blocks byte-identical after normalizing every checkbox to `[ ]`** (else a `-`/`+` diff); each item well-formed with ID components == depth+1; every leaf checked; no parent checked with an unchecked child; no parent unchecked with all children done; `TASK:` == README `id`; `STATE: DONE`; `CURRENT: NONE`; `BASELINE:` non-empty and not `<not run>`.

### 5.4 The threat model, as encoded in tests

`harness/tests/gate_tamper_matrix.rs` (19 tests) is the readable statement of what the gate defends against.

**progress.md tampering — all FAIL:**

| Scenario | Verdict |
|---|---|
| fresh, untouched `progress.md` | FAIL (`failed_checks == ["progress"]`) |
| fully-checked DONE file | **PASS** |
| `STATE` not DONE / `CURRENT` not NONE / `BASELINE: <not run>` / `TASK` id mismatch | FAIL |
| parent checked with unchecked child; parent unchecked with children done | FAIL |
| reworded checklist line; deleted line; added line | FAIL |
| missing progress file | FAIL |
| missing `verify.toml` | FAIL, exit 2 |

**Scope bypass — every attempt FAILs, and `scope` is *the* failing check:**

| Attempt | Evidence line |
|---|---|
| out-of-scope tracked edit | `OUTSIDE src/legacy/foo.rs` |
| out-of-scope untracked new file | `OUTSIDE …` |
| `git mv` out-of-scope → in-scope (rename hides the delete) | `OUTSIDE …` (gate uses `--no-renames`) |
| hide via `.git/info/exclude` | `OUTSIDE …` |
| untracked self-ignoring `.gitignore` in a new subdir | `OUTSIDE …/.gitignore` |
| `git update-index --skip-worktree` then edit | `HIDDEN index entries` + `S …` |
| `git update-index --assume-unchanged` then edit | `HIDDEN index entries` + `h …` |
| move the `baseline` tag past an out-of-scope commit | FAIL **because the recorded base SHA is pinned** — the same repo gated against `HEAD` passes, which is exactly why pinning is load-bearing |

`forbidden_paths` means *not created or modified by this run* — a trusted file present at base and untouched does not fail; editing it prints `CHANGED <path>`.

### 5.5 `verify` vs `gate`

Same engine, different framing.

- **`taskfmt verify`** is agent-facing, inside the container. It resolves root/task-dir/progress/base from flags and env, defaulting to the container layout (`/work`, `/task`, `/progress/progress.md`). Prints the transcript, returns the raw exit code.
- **`taskfmt gate <RUN>`** is host-facing. It re-runs the same engine against **trusted host copies**: `<run>/workspace` as root, `<run>/task-snapshot` as task dir (never touched by the agent), `manifest.base_sha` as base. It writes `out/gate.log`, records `manifest.gate = {verdict, exit, last_line, head, log, finished}`, and prints `GATE PASS|FAIL`.

That recorded verdict — not a re-runnable check — is what makes "never push on a failed gate" structural.

---

## 6. Layer 4 — the execution harness

### 6.1 Images

```
rust:1.98-bookworm ──build──▶ debian:bookworm-slim  =  harness-taskfmt:latest   (just the binary)
                                        │
debian:bookworm-slim + tools + herdr + rustup + COPY taskfmt  =  harness-base:latest
                                        ├──▶ harness-claude:latest  (npm @anthropic-ai/claude-code)
                                        └──▶ harness-codex:latest   (npm @openai/codex)
```

`harness-base` installs `git jq ripgrep iptables ipset iproute2 dnsutils gosu ca-certificates curl bsdutils build-essential gnupg docker.io` (the inner daemon for DinD; `bsdutils` supplies `script(1)`), `postgresql-client-16` from pgdg, Node 22, **herdr 0.8.2** (static binary), rustup 1.98.0 as user `agent`, and bakes `images/preload/postgres.tar` to `/opt/preload/`. Entrypoint: `["/usr/local/bin/taskfmt","container-entrypoint"]`.

`taskfmt preload` pulls `postgres:16-alpine`, writes the digest pin **write-once** to `images/preload/postgres.digest` (committed, 81 B), and `docker save`s the ~109 MB tar (gitignored) — D30. `build-images` **refuses to build the base** if that tar is missing or empty.

### 6.2 One run, step by step

Host side, before any container (`harness/src/cmds/run.rs`):

1. Resolve profile / model / effort; resolve the repo (recorded experiment state pins it).
2. Create `runs/<YYYYMMDD-HHMMSS>-<profile>-<TASK>/{workspace,task-snapshot,progress,agent-home,out,seed}`. Container name is `harness-<run id>`.
3. `git clone --branch main --single-branch <url> workspace/`; record `clone_sha`.
4. Copy `<task>/trusted/` over the clone, `git add -A`, `git commit -s -m "planner: <TASK> trusted material"`, `git tag baseline`. **That commit's SHA is `base_sha` — the scope base of record.**
5. Copy the task dir to `task-snapshot/`, top up `AGENTS.md`/`verify.toml` from the template, create the `CLAUDE.md → AGENTS.md` symlink.
6. `taskfmt lint` → `lint.log`; **abort on any ERROR**.
7. Optional `--selfcheck` → `selfcheck.log`; refuse on FAIL *and* on NOVERDICT.
8. `taskfmt progress-init` → `progress/progress.md`.
9. Copy `experiments/fixtures/seed/` → `seed/`.
10. Build the one-line `/goal` prompt from `harness/goal-prompt.md` (the first ```` ```text ```` fence whose info-string words contain the agent kind), scrub and save it to `prompt.txt`. Mint a session UUID.
11. Pre-seed `agent-home/` (Claude `settings.json` + `.claude.json`, or Codex `config.toml`) plus a `.gitconfig` with `safe.directory = *`.
12. Resolve `env_secret` via `op read`, write a **0600 env file** in `$TMPDIR`, then:

```
docker run -d --privileged --name harness-<run id> \
  -v <run>/workspace:/work  -v <run>/task-snapshot:/task:ro \
  -v <run>/progress:/progress  -v <run>/agent-home:/agent-home \
  -v <run>/out:/out  -v <run>/seed:/seed:ro \
  -e TASKFMT_BASE=<base_sha> -e AGENT_CMD=… -e AGENT_KIND=… \
  -e HERDR_SESSION=agent -e NET_MODE=all \
  -e <profile env_static…> --env-file /tmp/.taskfmt-env-<uuid> \
  --memory 4g --cpus 2 --pids-limit 2048  harness-claude:latest
```

**No `--rm`, no `-t`** — persistence is a hard rule, and the herdr server needs no TTY. The env file is deleted the instant `docker run` returns.

Container side, PID 1 as root (`container_entrypoint.rs`):

1. Install SIGTERM/SIGINT flag; remove a stale `/var/run/docker.pid`.
2. Spawn the inner `dockerd --storage-driver vfs` (D19: `vfs` avoids overlay-on-overlay), log to `/out/dockerd.log`, wait ≤60 s for `docker info`, `chmod 666` the socket.
3. Codex bootstrap if `CODEX_HOME` is set.
4. `chown agent /work /out /agent-home`.
5. **Prereq stage:** `docker load -i /opt/preload/postgres.tar` (no registry egress at run time), `docker run -d --name prereq-postgres -p 127.0.0.1:5432:5432 postgres:16-alpine`, `pg_isready` ≤60 s, restore seeds (`/work/**/tests/fixtures/seed.sql` then `/seed/*.sql`, sorted) via `psql -v ON_ERROR_STOP=1`. Success → `/out/prereqs.ready` + `/out/prereqs.json` (DSN password masked). **Failure → `/out/prereqs.FAILED` and then park forever** — the hard rule is never to exit on prereq failure, so the operator can attach and inspect.
6. `exec gosu agent taskfmt agent-launch`.

Agent launch, as user `agent`:

1. `herdr server` (does not daemonize) → `/out/herdr-server.log`; wait ≤10 s.
2. `herdr workspace create --cwd /work --label task --no-focus` → root pane id → `/out/pane-id`.
3. `herdr pane run <pane> "HERDR_AGENT=<kind> exec script -qfec '<AGENT_CMD>' /out/tui.log"` — `script(1)` captures the raw terminal stream from byte 0.
4. Supervise; on SIGTERM (`docker stop`) run `herdr server stop` then exit 0.

Host side, after `docker run`:

1. Poll `/out/prereqs.ready` up to `prereq_timeout_s` (180). On FAILED/timeout: dump the logs, **leave the container up**, exit 1.
2. Poll `/out/pane-id` (≤50 s), then `herdr agent rename <pane> task` (retried up to 120 s — the pane exists before herdr registers the agent record). Save `manifest.json`.
3. `herdr agent wait task --until idle --timeout 180000`.
4. `herdr agent prompt task "<prompt>"` (bracketed paste + Enter).
5. `confirm_acceptance`: 15 × 2 s looking for a transcript goal sentinel (Claude) or `agent_status == working` (Codex); re-send Enter at iteration 5 if the prompt is still visible.

Re-attach later: `taskfmt attach <RUN>` starts the container if stopped, then **`exec`s** `docker exec -it -u agent -e TERM=… <container> herdr`. **Detach is `ctrl+b q`, never `ctrl+c`.**

### 6.3 Mounts

| Host | Container | Mode | Contents |
|---|---|---|---|
| `<run>/workspace` | `/work` | rw | the fresh clone; `WORKDIR` |
| `<run>/task-snapshot` | `/task` | **ro** | README, AGENTS+CLAUDE, verify.toml, decisions |
| `<run>/progress` | `/progress` | rw | `progress.md` — the only agent-writable protocol file |
| `<run>/agent-home` | `/agent-home` | rw | `CLAUDE_CONFIG_DIR` / `CODEX_HOME`, transcript jsonl |
| `<run>/out` | `/out` | rw | dockerd/prereq/herdr logs, `pane-id`, `tui.log`; host also writes `screen.txt`, `gate.log`, `gate-logs/` |
| `<run>/seed` | `/seed` | **ro** | `*.sql` restored into postgres |

The read-only `/task` mount is the entire tamper defence for the contract (D1) — no parser, no hashing, no normalizer. The separate `/progress` mount exists so the agent never needs write access to a directory containing the contract.

### 6.4 herdr

One fixed session, `HERDR_SESSION=agent`. One workspace per container. The agent is **renamed to the stable target `task`** immediately after launch, and every subsequent host call addresses `task`, never the pane id (pane reads are the exception).

Host control surface, all via `docker exec -u agent -e HERDR_SESSION=agent`: `agent wait … --until idle|done|blocked`, `agent rename`, `agent prompt`, `agent send-keys`, `agent get`, `pane read --source visible`.

Hard-won constraint recorded in the code: `--source recent*` returns empty on headless Linux — never use it. Likewise `CLAUDE_CODE_DISABLE_ALTERNATE_SCREEN=1` is mandatory or the TUI output lives on the alt screen, invisible to scrollback.

### 6.5 Secrets

- `op read --no-newline <op://ref>` — the reference travels on argv (not secret); the **value comes back through a pipe** and is registered with the redactor before it is returned. `op`'s stderr is deliberately never printed, because `op` diagnostics can echo resolved material.
- The value reaches the container only through a `$TMPDIR/.taskfmt-env-<uuid>` file created `O_CREAT|O_EXCL` at **mode 0600**, deleted on `Drop`. `docker run` only ever sees `--env-file <path>`.
- Agent command lines carry no credential (asserted by test).
- Every stdout/stderr path and every artifact write goes through `redact::scrub` — `manifest.json`, `experiment.json`, `prompt.txt`, `screen.txt`, `gate.log`, `prereqs.json`.
- **Known gap:** files written *by the container directly onto the bind mounts* — `tui.log`, `herdr-server.log`, `dockerd.log`, the Claude session `*.jsonl` — never pass through the redactor. If the agent echoes `$ANTHROPIC_AUTH_TOKEN`, it lands unscrubbed on the host.

### 6.6 Completion detection

Trust order (`ops/transcript.rs`, `cmds/status.rs`): **transcript `goal_status` verdict** → the agent-authored `GOAL_RESULT status=` line in `tui.log` (label only, never load-bearing) → herdr's own `agent_status`.

Two corrections make this robust:

- A settled `IDLE`/`BLOCKED` is **downgraded back to `RUNNING`** when the transcript shows assistant activity inside a 300 s window, because herdr misreads Claude spinner frames.
- A terminal state must **hold through a 30 s settle after a 90 s warmup** before it is acted on.

`taskfmt status` emits one JSON line per poll and exits **0 when terminal, 3 otherwise**.

---

## 7. Layer 5 — orchestration

### 7.1 Repo lifecycle (D25)

A **disposable private GitHub repo** per experiment: `gh repo create <owner>/<repo_prefix>-<ts> --private --disable-issues`. A freshly created repo has no branches, so bootstrap works in a tempdir: `git init -b main` → `remote add origin` → `git commit -s --allow-empty -m bootstrap` → `git push -u origin main`.

**One task = one fresh container = one fresh clone.** Enforced structurally: there is no incremental path in the code at all. Every dispatch creates a new run dir and `git clone --branch main --single-branch` into it. Only `main` is ever used — no per-task branch, no PR flow. The local `baseline` tag is never pushed.

Repo pinning: with recorded experiment state, the recorded `repo_url` **always** wins, and a conflicting `--repo` is a hard error naming both. (This ordering fixed a real bug where resuming created a brand-new repo with none of the earlier commits.)

### 7.2 The experiment loop

`taskfmt experiment --tasks all --auto` (D26 — supersedes the earlier manual-trigger-only rule):

1. Load or create experiment state at `runs/<exp-id>/experiment.json`. **State is loaded before the repo is resolved.**
2. Resolve the task selection (`all`, `1-3,5`, `TASK-002..TASK-004`).
3. Skip tasks already recorded `pushed`.
4. **One confirmation for the whole batch**, printing one plan line per pending task. Declining exits 2.
5. Per task, in order: `dispatch_one` → `wait_and_gate` → `promotable = is_promotable(status) && gate.passed()` → `promote_run` → refresh the manifest → save state.

Three stop points, all after saving state, all exit 1: a dispatch error breaks the loop; a non-promotable task prints `stopping: <task> ended <state> with gate <verdict> — remaining tasks untouched` and returns immediately; a refused promote returns immediately.

`--auto` changes exactly two things: the batch confirmation is auto-answered yes (the plan still prints), and the non-TTY guard is lifted. The gate and the promote refusal are unchanged.

### 7.3 Promote — the only path to `origin/main`

`promote` refuses **before touching git** if: no workspace; no manifest; no recorded gate; `gate.verdict != "pass"`; workspace `HEAD != gate.head`; or `result_sha` already set. Only then, after confirmation:

```
git -C <run>/workspace add -A
git -c user.name=harness -c user.email=harness@localhost commit -s -m "<TASK>: <README title>"
git push origin main
```

A failed task never pushes. That is a structural property of the recorded verdict, not a policy statement.

---

## 8. Author-side tooling

Three commands, three different questions. They are frequently confused; they share nothing but the gate engine.

| Command | Question | Needs |
|---|---|---|
| `taskfmt lint` | *Is this package well-formed?* | nothing |
| `taskfmt selfcheck` | *Does this package's gate actually discriminate?* | a git workspace at the base commit; the fixture's toolchain |
| `taskfmt selftest` | *Is this binary and this repo sane?* | nothing — no config, no network, no toolchain |

### `selfcheck` (D13 → D36)

Three phases, run in a scratch copy under `$TMPDIR` — the caller's workspace is **never mutated**:

- **nop** — the gate must be RED (exit 1, ≥1 failing check) on the untouched base.
- **polarity** — every `[focused]` command must FAIL at base. `[regression]` results are **INFO only** (D28: under the empty-repo lifecycle a package ships its own new tests, so "regression PASS at base" cannot be required). A focused command with rc **126/127** is `NOVERDICT` (exit 69) — *"command not runnable: toolchain missing?"* — never counted as a RED.
- **oracle** — only with `--reference`: everything GREEN and the gate `DONE`. **No reference solution exists for any of TASK-001..007, so the oracle phase has never run.**

Exit codes: `0` PASS · `69` NOVERDICT (takes precedence over FAIL) · `1` FAIL · `66` missing input · `70` internal.

It is **opt-in at dispatch** (`--selfcheck`), because it runs the fixture's toolchain on the host; container-mode selfcheck is backlog item 24. A test pins the opt-in behavior.

### `selftest` — 31 repo-wide checks

1. **Every `AGENTS.md` has a sibling `CLAUDE.md` that is a symlink whose target is exactly `AGENTS.md`.** Never a real file, never the reverse direction.
2. The bundled example lints clean; `reference/task-template` does **not** (it still has placeholders, by design).
3. Ten lint mutants must all be rejected (broken grammar, non-contiguous IDs, H1/id mismatch, C1, C2, C3, C4, C5 ×2, C8).
4. `progress-init` output shape, with the checklist block verbatim.
5. The gate matrix: fresh progress fails; the DONE file passes; nine tampers all fail; a missing progress file fails.
6. `bash -eo pipefail` semantics — mirrored from the integration tests *so they are proven inside the image too*.

---

## 9. The corpus under test — `pgtui`, TASK-001..007

The fixture is **`pgtui`**, a Rust/ratatui terminal PostgreSQL browser. It exists to be *built by the agents*, one task at a time, from a repo whose `main` is a single empty commit.

**Stack, pinned with `=` everywhere** (D-001 in every `decisions.md`): `ratatui =0.30.2`, `crossterm =0.29.0`, `turso =0.7.2` (local SQLite store), `tokio =1.53.1`, `tokio-postgres =0.7.18` (**simple-query protocol only**, D-023), `clap =4.6.6`, `thiserror =2.0.20`, `directories =6.0.0`; dev: `tempfile`, `testcontainers` + `testcontainers-modules` (postgres:16-alpine), `nix`. Edition 2024, toolchain 1.98.0, `unsafe_code = "forbid"`. The planner owns `render.rs` + the bundled font via `resvg` — those are in `forbidden_paths` for every task.

The architecture is **fixed by decision, not left to the executor**: Elm-style core with a pure `App::update(&mut self, msg: Msg) -> Vec<Effect>` (no IO, no await) and `runtime::execute` doing the IO; one render entry point `ui::draw(app, buffer)` at a fixed 100×30 buffer. Roughly 70 planner decisions `D-001..D-072` nail down every screen layout, key binding, error string and exit code.

> **Two `D` namespaces exist.** Project decisions are `D1..D39` (in `RESEARCH-FINDINGS.md`). pgtui fixture decisions are `D-001..D-072` (in each task's `decisions.md`). They are unrelated.

Strictly linear; each task's `P-002` is "the previous task's gate is green":

| Task | Goal | New focused suites | Cumulative regression |
|---|---|---|---|
| **001** | bootstrap the Cargo workspace from an empty `main`; exit-2 stubs | build + skeleton + 3 grep pin checks | 4 |
| **002** | Turso store, connection-list screen, CLI loop | store / app / screen / cli | 20 |
| **003** | create-connection form and the save flow | app / runtime / screen create | 35 |
| **004** | connect to PostgreSQL, list tables (**Docker becomes a precondition**) | pg_connect / runtime / app / screen browser | 48 |
| **005** | preview grid + client-side sort | grid_sort / pg_preview / app / screen | 73 |
| **006** | custom SQL screen | app / pg / screen custom_sql | 90 |
| **007** | disconnect, exit codes in a real pty, SVG/PNG gallery, repo `README.md` | disconnect / cli_exit / gallery | 102 |

Every `verify.toml` shares a spine: `forbidden_paths = [render.rs, fonts]`, `[lint]` = `cargo fmt --all --check` + `cargo clippy --workspace --all-targets -- -D warnings`, and forbidden patterns for `todo!(`/`unimplemented!(`, `dbg!(`, `#[allow((clippy|dead_code|unused)`, `#[ignore]`. Task-specific additions encode decisions as greps: `tokio_postgres` banned in `src/` until 004; `query_one|query_raw|\.prepare\(` banned in `src/db/` from 004 (simple-query only); `sort_unstable` banned from 005 (sort stability).

---

## 10. Decision log — D1 to D39, condensed

Full statements and rationale: `docs/research/RESEARCH-FINDINGS.md:59-98`.

| ID | Decision |
|---|---|
| D1 | `README.md` byte-immutable on a read-only mount; checkboxes live in a generated `progress.md` the gate diffs against it |
| D2 | Split task content (`README.md`) from protocol (`AGENTS.md`) — replaces v2's ~75–80 rules / ~7.5k tokens per task |
| D3 | Keep the nested checklist: IDs to depth 4, leaf-only counting, one `CURRENT` leaf, parents roll up |
| D4 | Drop every hand-maintained counter, percentage and timestamp — five projections of one state diverge |
| D5 | Remove non-verifiable checklist leaves ("read context", "prepare report") |
| D6 | The gate leaf is last; the harness re-runs the gate on the final tree and that run is the verdict |
| D7 | Keep a short per-turn `GOAL_PROGRESS` line + one terminal `GOAL_RESULT` |
| D8 | Trim the completion report to nine fields |
| D9 | Merge acceptance criteria and evidence into one table |
| D10 | Every precondition has a command that exits 0 when true — otherwise it is context |
| D11 | Frontmatter reduced to `id/title/kind/verify/expected_paths` |
| D12 | A concrete generic verifier plus data-only config; structured `CHECK/SUMMARY/RESULT`, `DONE` only on a full pass |
| D13 | **Oracle required at dispatch**: the gate must FAIL on the untouched fixture and PASS on a reference |
| D14 | Container layout `/task` ro, `/work` rw, `/progress` rw; the package never lives inside the repo |
| D15 | `reference/task-template/` holds only agent-visible files; all tooling lives in `harness/` |
| D15b | Launch prompt is a short `/goal` condition (<4,000 chars) with an explicit turn bound |
| D16 | Hooks are not relied on as gates — the host gate is the authority |
| D17 | Default ID order; no over-prescriptive step sequence |
| D18 | The compat-layer prohibition is task content (`R-004`), not protocol |
| D19 | One persistent `--privileged` container per run with an inner dockerd (`vfs`) for testcontainers |
| D20 | Images preload `postgres:16-alpine`; the entrypoint keeps a seeded standing instance on loopback |
| D21 | Egress firewall **deferred** — the inner dockerd shares the netns, so a default-DROP ruleset breaks DinD |
| D22 | Model **and effort** pinned per run and recorded |
| D23 | **Whitelist only.** `expected_paths` ≡ `allowed_globs`; the blacklist and hash manifest are removed |
| D24 | **One Rust CLI `taskfmt`** — no executable shell script in the execution path |
| D25 | Disposable GitHub repo, allow-empty bootstrap `main`, fresh clone per task, trusted overlay commit as the scope base, push only on PASS |
| D26 | `taskfmt experiment --auto` may run the whole sequence unattended; confirmation stays the interactive default |
| D27 | Versioned agent profiles; default `zai-flash` (Z.ai endpoint, GLM-5.3-Flash, effort low); token via 1Password into a 0600 env-file only |
| D28 | TASK-001..007 from an empty `main`; application code is never fixture-supplied; trusted tests use **behavioral assertions**, not golden snapshots |
| D29 | Gate config becomes `verify.toml` (data only); `EXTRA_CHECKS` bash hooks dropped |
| D30 | Preload tar gitignored, digest pin committed, `taskfmt preload` regenerates |
| D31 | **Two-phase gate run** (8a `--progress ""`, then 8b full) — fixes a step 8 that was literally impossible |
| D32 | New terminal status **`INCOMPLETE`** for turn/budget caps |
| D33 | Scope check fail-closed (`--no-renames`, untracked enumeration, skip-worktree/assume-unchanged detection, `bash -eo pipefail`) + base pinned to the recorded SHA |
| D34 | **Per-AC proof leaves dropped**; each AC cited on the item whose evidence is that AC's command |
| D35 | Binding vs orientation separated: hints never reference `/task/*`; `decisions.md` is binding when present |
| D36 | `taskfmt selfcheck` automates D13 (nop / polarity / oracle); polarity lives in the harness, not in README columns |
| D37 | Lint rules C1–C9 |
| D38 | Protocol polish: BLOCKED vs NEEDS_REPLAN, resume keyed on `## Log`, re-read after compaction, `CHANGED:` derived from git, `NEXT:` dropped |
| D39 | Codex dispatch corrected: per-kind prompt fence, Codex-aware `status` output |

**Numbering caveat.** The v3.1 branch numbered these D24–D32; they were remapped to D31–D39 in the findings. `notes/synthesis-2026-08-28.md` uses the **old** numbering throughout its body — a reader entering mid-file will misattribute decisions.

---

## 11. Current state — verified health

Measured in this analysis, from a freshly rebuilt binary:

| Check | Result |
|---|---|
| `cargo fmt --check` | **PASS** |
| `cargo clippy --all-targets -- -D warnings` | **PASS**, 0 warnings |
| `cargo test` | **PASS** — 191 passed, 0 failed, **0 ignored** |
| `taskfmt selftest` | **PASS** — 31/31 `ok` |
| `taskfmt lint reference/task-template` | **FAIL, by design** — errors=3, all `placeholders`/`config` placeholder errors (asserted expected) |
| `taskfmt lint experiments/tasks/TASK-001..007` | **PASS** all 7. TASK-001 has 2 C6 warnings (compound `grep` AC commands not run by `verify.toml`); 002–007 clean |

Test suites: lib 101, `lint_corpus` 22, `gate_tamper_matrix` 19, `experiment_resume` 12, `selfcheck` 12, `run_resolve` 10, `secrets_and_consent` 8, `config_selection_interactive` 5, `docker_itest` 2.

**What is *not* proven by any of that:** agent behavior. There has been **no measured experimental run** (`RESEARCH-FINDINGS.md:226`):

> No measured run exists — TASK-001 was dispatched during harness debugging, not as an experiment; **every predictability claim in D31–D39 is a hypothesis.**

---

## 12. Gaps, defects and contradictions

Everything here is evidence-backed from the current tree. Roughly ordered by consequence.

### 12.1 The one recorded run contains a decision override that the gate could not see

`experiments/runs/20260828-175916-zai-flash-TASK-003/` **passed** the gate (11/11 checks, `RESULT PASS`, `DONE`). Its `progress.md` handoff records:

> `DECISIONS:` validation required-check is Name only + port … the trusted `screen_create_form_test` saves a form with only Name filled, which is incompatible with the D-032 all-fields-required reading; **trusted tests were treated as the oracle**.

Per `AGENTS.md`, a contradiction between a fixed decision and the acceptance material is a `NEEDS_REPLAN` stop condition. The agent instead resolved it unilaterally, documented it, and passed. **The gate has no check for decision conformance beyond the trusted tests** — which is exactly the gap the D-032 case exposes.

A **later run of the same task with the same model failed** on that same trusted test (`experiments/runs/exp-20260828-165259/`: `"gate": "fail"`, `"pushed": false`; `focused.3 FAIL … index out of bounds: the len is 0 but the index is 0`). Same task, same model, opposite outcomes. That is the single most interesting datum in the repo, and it points at a package defect (a contradictory D-032), not at agent variance alone.

### 12.2 The recorded run cannot be reproduced from the current package

Diffing `experiments/tasks/TASK-003/` against the run's `task-snapshot/`:

- The snapshot's `AGENTS.md` is materially **older** than today's template — it lacks the D31 two-phase endgame, the D32 `INCOMPLETE` status, the D38 git-derived `CHANGED:`, and the "passes and fails on the same tree" rule; it still has `NEXT:`.
- The README's checklist section 5 has since been split from 2 leaves into 3 — the run's `progress.log` reads `leaves=10 checked=10`, but the current package has 11.
- `prompt.txt` (695 B) predates two edits to `harness/goal-prompt.md` (now 785 B).

Because `experiments/runs/` is gitignored, `task-snapshot/` is the **only** surviving record of what was dispatched. There is no provenance link from a run to the template/prompt version it used.

### 12.3 Real defects in the code

| # | Defect | Evidence |
|---|---|---|
| 1 | **`taskfmt codex-login` is not a subcommand.** The container entrypoint runs `gosu agent taskfmt codex-login` and `codex_login()` is implemented, but `cli.rs` has no such variant — clap rejects it. **Codex API-key seeding is broken.** | `cmds/container_entrypoint.rs:70-82` vs `cli.rs` |
| 2 | **`run` has no consent gate.** The root help says `run` needs `--auto`/`--yes` on a non-TTY, but `cmds/run.rs` never calls `require_consent_source`, and with `--repo` supplied `ensure_repo` returns immediately. `taskfmt run --task X --repo URL` starts a privileged container on a pipe with no confirmation. | `cmds/run.rs`, `cmds/repo.rs:85-87` |
| 3 | **`repo create` / `repo delete` discard the confirmation result.** Answering `n` on a TTY still creates — or **deletes** — the repo. | `cmds/repo.rs:26`, `:54-57` |
| 4 | **Global `--yes` does not reach `promote` / `repo delete`.** Both define a local `--yes` that shadows the global; `promote_run` builds its `Interaction` from `auto \|\| <local yes>`. `--auto` works everywhere; `taskfmt --yes promote R` does not. | `cmds/promote.rs:63-64` |
| 5 | **Container-written logs bypass the redactor** — `tui.log`, `herdr-server.log`, `dockerd.log`, the Claude session `*.jsonl` land on bind mounts unscrubbed. | §6.5 |
| 6 | **`manifest.pane` goes stale across a restart.** `attach` does `docker start`, which re-runs the entrypoint and creates a *new* pane, but never re-reads `/out/pane-id` or re-runs the rename. `script -qfec` also **truncates** `tui.log`, and `docker rm -f prereq-postgres` discards all DB state. | `cmds/attach.rs:22` |
| 7 | **`docker_itest.rs` is silently inert.** Both tests early-`return` (reported *passed*) unless `TASKFMT_ITEST_DOCKER=1` and the images exist. `#[ignore]` would make "not exercised" visible. | `tests/docker_itest.rs:16` |
| 8 | **`lint` requires `experiment.toml` in the cwd** even with an explicit directory argument — `Ctx` loads the manifest unconditionally, so a read-only, config-independent command is cwd-sensitive and exits 1 outside the repo root. | `cmds/mod.rs` |
| 9 | **`[prereq]` in `experiment.toml` is dead config** — parsed but never read; the entrypoint hardcodes the image/user/password/db. Same class: `CLAUDE_CODE_VERSION`/`CODEX_VERSION` live only in Rust and Dockerfile ARGs. | `container_entrypoint.rs:17-22` |
| 10 | **`NET_MODE=all` is dead** — set on `docker run`, read nowhere. `iptables`/`ipset` are installed but no egress control exists, and no `--network` flag is passed (D21 documents the deferral; the env var is vestigial). | `ops/container.rs:125` |
| 11 | **`selfcheck` exit code 64 is unreachable** — advertised in the help, never returned. | `selfcheck.rs:38` |
| 12 | **Run-id collisions are possible** — 1-second granularity; two same-second dispatches of the same profile+task collide on `docker run --name`. | `runstate.rs:192-194` |
| 13 | `verify` with no `--log-dir` creates a temp dir and calls `.keep()` — per-check logs accumulate in `$TMPDIR` forever. | `gate.rs:222` |

### 12.4 Known coverage holes in the design itself

- **The oracle phase has never run.** No reference solution exists for any of TASK-001..007, so `selfcheck --reference` is untested against the real corpus.
- **TASK-001 cannot be proven RED at its base** — its focused commands are not runnable in an empty repo (no crate) ⇒ `NOVERDICT` (exit 69). Fixing this needs the container-mode selfcheck (backlog 24).
- **The `.gitignore` scope hole** (backlog 26): a tracked, whitelisted `.gitignore` edited by the agent can hide new files from the untracked enumeration. The fix is to enumerate with the *base commit's* ignore rules. Not in the tamper matrix.
- **`cheat_seed_gate_pass` must be 0 before any format comparison is trusted** (backlog 18). Harbor-style adversarial seeds have not been run. Note the self-criticism already in the findings: `forbidden_patterns` greps are exactly the "keyword" class Harbor's rubric rejects — they are structural guards, not primary proof.
- **Codex path is largely unverified**: `/goal` via `herdr agent prompt`, `/goal clear` on kill, goal events in the rollout JSONL. And the `codex-default` profile **declares no `env_secret`**, so a Codex run cannot authenticate at all.
- **Not fixable by formatting** (stated explicitly in the findings): transcript `DONE` forgery against the Haiku evaluator (the host gate is immune; `false_done` counts it), the sensitivity of agent-authored `#[cfg(test)]` tests, and the headed turn cap being model-judged because `--max-turns` is print-mode only.

### 12.5 Documentation drift

- Root `README.md:29` says "Decisions + evidence: … (D1–D30)" — stale; the findings carry **D1–D39**.
- `notes/example-app-decomposition.md` and `notes/headed-herdr-harness.md` bodies still describe `protected_paths` / hash manifests (superseded by D23). Both carry a banner at line 1; the bodies were deliberately left as written.
- `harness/images/codex/codex-config.toml:2` still says "Copied … by entrypoint.sh" — a stale reference to a removed shell script. That file also exists in three places that must agree (base Dockerfile, codex Dockerfile, and a hand-maintained copy in `ops/container.rs`).

---

## 13. Operator runbook

```bash
# 0. one-time: bake the postgres tarball, then build the four images
taskfmt preload
taskfmt build-images --agent claude

# 1. author-side checks (from the repo ROOT — see defect 12.3 #8)
taskfmt lint                                  # every package under tasks_dir
taskfmt selftest                              # repo + binary invariants
cd harness && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test

# 2. create the disposable experiment repo (bootstraps an empty signed main)
taskfmt repo create

# 3a. one task, attended
taskfmt run --task TASK-001 --repo <url>
taskfmt status <RUN> --wait                   # exits 0 when terminal, 3 otherwise
taskfmt attach <RUN>                          # detach with ctrl+b q — NEVER ctrl+c
taskfmt gate <RUN>                            # host gate; records the verdict
taskfmt promote <RUN>                         # refuses on anything but a PASS

# 3b. the whole sequence, unattended
taskfmt experiment --tasks all --auto

# 4. inspect
cat experiments/runs/<RUN>/manifest.json
cat experiments/runs/<RUN>/out/gate.log
ls  experiments/runs/<RUN>/out/gate-logs/
less experiments/runs/<RUN>/progress/progress.md
```

Run containers are **never removed by the harness** — `docker rm -f harness-<run id>` is the operator's call, deliberately, so a failed run stays inspectable.

---

## 14. What "run the template on itself" would mean

The stated next step is to use `reference/task-template/` to drive changes *to this project*. That is coherent, and this document is the prerequisite for it. Concretely it requires four things the repo does not have yet:

1. **A second corpus.** `experiments/tasks/` is pgtui-specific. Self-hosted tasks would need their own directory, and `experiment.toml`'s `paths.tasks_dir` is a single value — either a second manifest or a new path key.
2. **A `verify.toml` for this repo.** The natural gate spine already exists: `[lint]` = `cargo fmt --all --check` + `cargo clippy --all-targets -- -D warnings`; `[regression]` = `cargo test` + `taskfmt selftest`; `[focused]` = whatever new test the task adds. `forbidden_paths` would cover `reference/task-template/` for most tasks — the template is the research variable and must not drift under an executor.
3. **A base commit and a disposable repo.** The one-task-one-clone rule means the executor works on a clone of *this* repo, not on the working tree. That is a mirror repo, not `origin`.
4. **A reference solution per task**, if the oracle phase is ever to run — the gap noted in §12.4.

The defects in §12.3 are the obvious first candidates for such tasks: each is small, has a clear RED baseline (a failing test can be written first), and has an unambiguous `expected_paths` whitelist. Defect #1 (`codex-login` missing from the CLI) and defect #2 (`run` has no consent gate) are the two that change behavior rather than hygiene.

---

## Appendix — schema and constant reference

| Marker | Value | Defined at |
|---|---|---|
| Task schema | `task/v4` | `harness/src/lint.rs:53` |
| Gate config schema | `verify/v1` | `harness/src/verifycfg.rs:8` |
| Experiment manifest schema | `experiment/v1` | `experiment.toml:6` |
| Gate command | `taskfmt verify` | `harness/src/lint.rs:55` |
| Checklist markers | `<!-- checklist:start -->` / `<!-- checklist:end -->` | `harness/src/taskfile.rs:10-11` |
| Default scope base tag | `baseline` | `harness/src/gate.rs:27` |
| Scope base env | `TASKFMT_BASE` | `harness/src/gate.rs:314` |
| Template override env | `TASK_TEMPLATE_README` | `harness/src/lint.rs:57` |
| Pass signal | exit 0 **and** last line `DONE` | `harness/src/gate.rs:82-87` |
| Gate exit codes | 0 pass · 1 check failed · 2 no config · 70 internal | `harness/src/gate.rs` |
| Selfcheck exit codes | 0 pass · 1 fail · 66 missing input · 69 noverdict · 70 internal | `harness/src/selfcheck.rs` |
| Status exit codes | 0 terminal · 3 not terminal | `harness/src/cmds/status.rs` |
| Run id | `<YYYYMMDD-HHMMSS>-<profile>-<TASK>` | `harness/src/runstate.rs:192` |
| Container name | `harness-<run id>` | `harness/src/ops/container.rs:18` |
| herdr session / agent target | `agent` / `task` | `harness/src/ops/herdr.rs` |
