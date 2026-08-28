# Research findings — task structure for coding agents (source of truth)

Status: 2026-08-28. Consolidates `raw/01`, `raw/02` (chronological inputs; later supersedes earlier) and `notes/*` (sub-agent research: external evidence, adversarial critique, container harness, progress/verifier design). Where inputs disagree, the decision and its reason are recorded here. This file wins over every other document in the repo.

Project goal: find the task-package structure that gives the most predictable output from one coding agent (Claude Code `/goal` first, Codex second) handed one bounded task in a fresh isolated Docker container. Method: one structure at a time, run, analyze, iterate; every run triggered manually.

---

## 1. Problem statement (stable across all inputs)

Long `/goal` runs drift when they combine: several independently completable outcomes; unresolved decisions; unrelated repo context; repeated compaction; weak/subjective completion criteria; self-verification by the implementer. Anthropic observed exactly this (context exhausted mid-feature, later agent declares job done) and fixed it with one feature per session + persistent progress artifacts + explicit verification. [A1] Codex guidance: a goal is "bigger than one prompt but smaller than an open-ended backlog." [O2]

Architecture (unchanged since raw/01):

> Big goal → planned task DAG → one ready task → one fresh context → one isolated workspace → deterministic verification → independent completion decision.

The DAG/orchestrator is out of scope for now. This project studies the single-task package.

---

## 2. Verified platform facts (drive design; from notes/external-evidence.md, notes/container-harness.md)

### Claude Code

- `/goal` = session-scoped **prompt-based Stop hook** on a small model (Haiku by default). After each turn it judges the condition against **the transcript only** — it runs no commands, reads no files. Condition ≤ 4,000 chars. Docs recommend one measurable end state + a stated check + constraints + a turn clause ("or stop after 20 turns"). Loop stops on: met, judged impossible, several consecutive turns with no tool use, auth/credit failure, unrecoverable context overflow. [C1]
- `/goal` **works non-interactively**: `claude -p "/goal <condition>"`; watch with `--output-format stream-json --verbose`. [C1] (raw/01–02 and the critique assumed otherwise; superseded.)
- `--bare` skips hooks → `/goal` under `--bare` UNVERIFIED (likely unavailable); `--bare` also ignores OAuth token. Do not use `--bare` for `/goal` runs.
- Headless flags verified: `-p`, `--output-format json|stream-json`, `--allowedTools`, `--permission-mode`, `--dangerously-skip-permissions` (rejected as root → non-root container user), `--max-turns`, `--max-budget-usd` (print mode only), `--model`, `--session-id`, `--add-dir`, `--settings`, `--append-system-prompt-file`, `--json-schema`. `result` event carries `total_cost_usd`, `num_turns`, `usage`. [C5]
- Hooks: `Stop` can block (`decision: "block"` + `reason`) but Claude Code overrides after 8 consecutive blocks (`CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`). **`PostToolUse` cannot block.** `PreToolUse` `deny` works even under `--dangerously-skip-permissions`. `SessionStart` matcher `compact` re-injects context (reliability UNVERIFIED; issue #15174). CLAUDE.md is advisory context, re-read after compaction; HTML comments stripped on the CLAUDE.md path but NOT when a file is read with the Read tool. [C4][C3]
- Instruction-file sizing: "target under 200 lines"; "Bloated CLAUDE.md files cause Claude to ignore your actual instructions"; "If you emphasize many lines, none of them stands out." [C2][C3]
- Transcript: `$CLAUDE_CONFIG_DIR/projects/<cwd-dashed>/<session-id>.jsonl`; with `--session-id` the path is known before the run; written asynchronously — copy after exit.
- Official devcontainer: `node:20`, non-root `node`, `init-firewall.sh` (iptables default-DROP + allowlist; needs `NET_ADMIN`/`NET_RAW`). Docs bless `--dangerously-skip-permissions` there. [C6]
- Auth precedence in `-p`: `ANTHROPIC_API_KEY` > `CLAUDE_CODE_OAUTH_TOKEN` (`claude setup-token`). Never mount host `~/.claude`.

### Codex CLI (0.150.1)

- `codex exec [--json] [-o file] [--sandbox ...] [-C dir] [--add-dir] [--skip-git-repo-check] [-c k=v]`; `--full-auto` deprecated. In Docker, official guidance: bwrap often fails → `--dangerously-bypass-approvals-and-sandbox`, Docker is the sandbox. Auth: `CODEX_API_KEY`. Rollouts: `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-*.jsonl`. AGENTS.md: git root → cwd, 32 KiB cap. [O7]
- Goals in `exec`: no `--goal` flag; maintainer says experimental, not first-class. Harness v1: one `exec` = one attempt; optional `codex exec resume --last` retries. [O1]
- Codex goal anatomy (six elements): outcome, verification surface, constraints, boundaries, iteration policy, blocked stop condition. Anti-patterns: vague finish lines, no auditable completion. [O1][O2]

### Eval harnesses (how the field hands tasks to agents)

- SWE-bench: agent sees repo at `base_commit` + `problem_statement` (+ hints); hidden gold patch, `FAIL_TO_PASS`/`PASS_TO_PASS`; grader re-applies the test patch in a fresh container (agent's test edits do not count). [E1]
- Terminal-Bench/Harbor: `instruction.md` + `task.toml` + `environment/Dockerfile` + hidden `tests/test.sh` writing `/logs/verifier/reward.txt` + **mandatory oracle `solution/solve.sh`**; instruction describes the end state, not steps; adversarial reward-hack review. [E2]
- Anthropic evals: "two domain experts would independently reach the same pass/fail verdict"; build a reference solution to prove the grader; grading exact tool-call order is "too rigid". [A5]
- "What makes a good terminal-agent benchmark task": ~two paragraphs "as if for a smart human"; tests verify outcomes not implementations; >15% of tasks in popular benchmarks reward-hackable. [E3]
- No peer-reviewed comparison of structured vs prose task specs was found. This project's experiments fill that gap for our own use.

### Vendor guidance (Copilot, Devin, Amp)

Converges on: clear problem, acceptance criteria, file pointers, concrete success checks ("returns 200"), one thread per task, no significant design decisions left to the agent. [V1][V2][V3]

---

## 3. Decisions (what raw/02 v2 said → what v3 does, and why)

| # | Topic | v2 (raw/02) | v3 decision | Reason / evidence |
| --- | --- | --- | --- | --- |
| D1 | Where checkbox state lives | Inside `README.md` in marked mutable regions; normalizer before hashing | **`README.md` byte-immutable, read-only mount. Checkboxes live in `progress.md` as a verbatim generated copy of the checklist block; the gate diffs it (modulo `[ ]`/`[x]`) against `README.md`.** | Read-only mount is the only parser-free tamper protection (raw/01). Anthropic: agents "inappropriately change or overwrite" markdown they may edit [A1]. Tested: reworded/reordered copy fails the gate. Keeps your requirement (visible numbered checkboxes) without giving the agent write access to the contract. |
| D2 | Protocol location | Repeated in every `README.md` (~75–80 rules, ~7.5k tokens) | **Split: `README.md` = task-specific content only (~1.1k tokens template); `AGENTS.md` = protocol shared by all tasks (~1k tokens).** | Every instruction-file source warns adherence drops with length [C2][C3][O4]. Split makes protocol an independent experimental variable (same AGENTS.md, vary README.md, and vice versa). |
| D3 | Checklist depth / numbering | 4 levels, four-space indent, leaf-only progress, one current leaf | **Kept: IDs `N`…`N.N.N.N`, max depth 4, four spaces per level, leaf-only counting, exactly one `CURRENT` leaf, parents roll up.** Depth 4 is a ceiling, not a target; no single-child levels for show. | Your requirement. GFM task lists are binary; three states via the `CURRENT` pointer [G1]. Depth cap is a task-sizing signal (raw/02). Critique's "flat or 2-level" is recorded as a future ablation (§7), not adopted now. |
| D4 | Hand-maintained counters | `Leaf progress n/N (x%)`, `LEAVES:`, per-turn counts, report `CHECKLIST` block, timestamps | **Dropped.** Agent writes only checkbox tokens, `STATE`, `CURRENT`, `BASELINE`, log lines, handoff. Counts/percent derived by the gate/harness. No timestamps (harness has them; agents fabricate). | Critique: five projections of one state diverge; v2's own two progress-log copies already differed. |
| D5 | Checklist leaves that are not verifiable | 1.1 "read context", 1.2 "confirm preconditions", 4.4 "reconcile", 4.5 "prepare report" | **Removed.** Kept: preconditions leaf (commands exist), baseline leaf, implementation leaves, AC proof leaves, diff review leaf, gate leaf. | Unverifiable leaves inflate progress and are invisible to any gate. |
| D6 | Gate leaf circularity | verify.sh checks checklist consistency, but 4.3 runs it before 4.4/4.5 are checked → cannot pass | **Gate leaf is last (`4.2`). Agent runs verify.sh, fixes until it passes except the `progress` check, then sets `STATE: DONE`, checks remaining leaves, reruns once.** Harness re-runs verify.sh on the final tree; that run is the verdict. | Only an outer run can bless the final tree. |
| D7 | Per-turn signal | `GOAL_PROGRESS` with counts | **Kept, shortened:** `GOAL_PROGRESS task= state= current= done_this_turn= blocked=`. Plus one terminal `GOAL_RESULT` line. | `/goal` evaluator sees only transcript [C1]; Codex asks for checkpoint reports [O2]. Critique's "no consumer" argument was wrong because `/goal` works in `-p`. |
| D8 | Completion report | 7 blocks incl. `CHECKLIST`, `CONTRACT_INTEGRITY` | **Trimmed:** STATUS, TASK, SUMMARY, ACCEPTANCE, VERIFY, CHANGED, DEVIATIONS, FOLLOW_UP, `GOAL_RESULT`. | Agent cannot know contract integrity; harness hashes. Derivable blocks dropped. |
| D9 | Acceptance criteria + evidence map | Two sections (GWT blocks + separate table) | **One table:** ID, Given/When/Then, evidence command, expected. | Two representations diverge; ~500 tokens saved. |
| D10 | Preconditions | Prose | **Each has a command that exits 0 when true.** No command → it is context, not a precondition. | Harbor/SWE-bench make environment executable [E2][E1]. Mechanical `BLOCKED`. |
| D11 | Frontmatter | schema, parent, depends_on, base_ref, subsystem, progress_log, task_contract, checklist config, verification, protected_paths | **Kept: `id`, `title`, `kind`, `verify`, `expected_paths`.** Scope is the whitelist alone (D23). DAG metadata belongs to the orchestrator (later). | Fields with no in-container consumer are attention cost. |
| D12 | verify.sh | Conceptual sketch | **Concrete generic script + `verify.config`.** Structured `CHECK <name> PASS|FAIL` lines, `SUMMARY`, `RESULT`, `DONE` last line only on full pass; ERR trap → never a silent DONE. Checks: scope (git diff vs BASE_REF ∪ staged ∪ untracked vs ALLOWED globs — whitelist only, D23), required/forbidden paths, forbidden patterns, focused/regression/lint commands, progress consistency. Same script for agent feedback loop and host gate (env vars point at trusted copies). | Tested on a fixture: baseline FAIL rc=1; solved+DONE rc=0; parent/child, reworded copy, out-of-scope, outer-gate invocation all correct. |
| D13 | Oracle requirement | Absent | **Required at dispatch:** verify.sh must FAIL on the untouched fixture and PASS with a reference solution. | Harbor mandatory `solve.sh`; Anthropic "reference solution proves the grader" [E2][A5]. |
| D14 | Container layout | `tasks/TASK-042/` inside repo | **`/task` (ro: README.md, AGENTS.md, verify.sh, verify.config), `/work` (rw fresh fixture copy, `baseline` tag), `/progress` (rw: progress.md).** Task package never inside the repo. | Package inside the repo pollutes the scope diff and can be `git add`ed. Separate `/progress` dir avoids the UNVERIFIED file-bind-over-ro-dir trick. |
| D15 | Repo layout (2026-08-28) | dispatch tools inside `reference/task-template/` | **`reference/task-template/` holds only agent-visible package files (README.md, AGENTS.md+symlink, verify.sh, verify.config). All tooling (task-lint.sh, progress-init.sh, selftest.sh), the launch prompt and the lint corpus live in `harness/`.** | The template folder is the research object; anything else in it is a variable the agent can see. |
| D15 | Launch prompt | ~180-word `/goal` prose, no turn bound | **Short condition (<4,000 chars) naming: verify.sh run+DONE shown in transcript, progress STATE DONE, final report, nothing under /task changed, BLOCKED/NEEDS_REPLAN rules, "stop after 40 turns"; plus `--max-turns 40 --max-budget-usd 10`.** | `/goal` docs [C1]. |
| D16 | Hooks as gates | v1 implied PostToolUse gates | **Not relied on.** Optional later: `PreToolUse` deny on `/task/*` edits, `SessionStart compact` re-injection. The host gate is the authority. | `PostToolUse` cannot block; Stop hooks capped at 8 [C4]. |
| D17 | Ordering rule | "numeric depth-first, do not skip" as MUST | **Default ID order; `README.md` may state a different dependency order.** | Over-prescriptive step order is an anti-pattern [A5][E3]. |
| D18 | Compat-layer prohibition | In protocol prohibited list | **Moved to `README.md` as `R-004` (task content).** | It is a requirement, not protocol. |
| D19 | Container privilege model | headless plan: one plain container per run, API-only firewall | **One persistent `--privileged` container per run (still no `--rm`) running an inner dockerd (storage `vfs`) for testcontainers.** | pgtui gates need real PostgreSQL via testcontainers-rs (§5b) and the agent container must stay self-contained; `vfs` avoids overlay-on-overlay; the prereq stage proves the stack before the agent starts (`/out/prereqs.ready`). |
| D20 | PostgreSQL prereq | agent tests pull/start postgres themselves at run time | **Images preload `postgres:16-alpine` (`preload.sh`, inner `docker load`); the entrypoint keeps a standing `prereq-postgres` (`pgtui/pgtui/pgtui` on `127.0.0.1:5432) seeded from `/work/**/tests/fixtures/seed.sql` then `/seed/*.sql` (default `experiments/fixtures/seed/`). Hermetic per-test testcontainers remain the canonical gate.** | No registry egress at run time; a seeded DB exists even for empty-fixture smoke runs; seed data identical to the fixture spec (D-072). |
| D21 | Egress firewall | API-only iptables allowlist at container start (v1 plan) | **Deferred: `NET_MODE=all` for v1.** The inner dockerd shares the outer container's netns, so an outer default-DROP ruleset breaks DinD. | Structural conflict, not a missing feature; revisit with netns separation or a proxy sidecar. |
| D22 | Model / effort pinning | model only, effort unpinned | **`run-headed.sh --model M --effort E`, default effort `high`; claude: `--model` (default `sonnet`) + `--effort` + `CLAUDE_CODE_EFFORT_LEVEL` env; codex: `-m` when set + `-c model_reasoning_effort`. Recorded in `meta.json`.** | Effort shapes output as strongly as model; it is an experiment variable and must be pinned per run. |
| D23 | Scope protection (2026-08-28) | `protected_paths` blacklist hashed at dispatch (`protected.sha256`, `manifest.sh`, `hash-protected.sh`) + `DENIED_GLOBS`, alongside the `expected_paths` whitelist | **Whitelist only: `expected_paths` (glob patterns, `*`/`**` cross `/`) equals `ALLOWED_GLOBS`; every changed file outside it fails the gate. Blacklist, per-file hash manifest and their tooling removed. `/task` integrity is the read-only mount.** | Two lists (allow + deny) duplicate one decision and drift; on a big repo the deny list grows without bound while the allow list stays small. One list, one enforcement point (`check_scope`). A changed file outside the whitelist fails the gate whether it was "protected" or simply unknown — same signal, no enumeration. |
| D24 | Execution tooling (2026-08-28) | bash scripts (`harness/*.sh`, `harness/images/common/*.sh`, template `verify.sh`) | **One Rust CLI, `taskfmt` (crate `harness/`). Lint, progress-init, gate/verify, selftest, image build/preload, repo lifecycle, dispatch, attach/status, experiment loop, container entrypoint/prereqs/agent-launch — all Rust; the binary is baked into the images (`ENTRYPOINT ["/usr/local/bin/taskfmt", "container-entrypoint"]`). No executable shell script remains in the execution path; Dockerfiles and declarative config/data (verify.toml, experiment.toml, goal-prompt.md) stay.** | One language, one test surface (cargo test), type-checked orchestration, testable git/docker/herdr wrappers; bash ports drifted and were untestable without throwaway fixtures. |
| D25 | Repository lifecycle (2026-08-28) | per-run workspace git-inited from a fixture with a `baseline` tag; no remote; nothing pushed | **The experiment runs against a real disposable GitHub repo whose `main` starts as one allow-empty bootstrap commit. Per task: fresh clone (or fetch+reset+clean of a run workspace) of `origin/main`, record base SHA, overlay the task's `trusted/` material, commit it as the run-local trusted base commit (NEVER pushed), gate against that commit, and only on PASS `git commit -s` + push `main`. Failure never pushes. The next task always starts from a newly fetched remote `main`.** | Proves the format end to end from a truly empty repo; the trusted commit gives the gate a stable scope base (SWE-bench's base_commit + test patch, adapted) and makes trusted tests unmodifiable without failing the scope check; per-task fresh fetch removes cross-task state leakage by construction. |
| D26 | Trigger policy (2026-08-28) | hard rule: every run triggered manually; no autonomous orchestration | **Superseded: `taskfmt experiment --auto` runs the whole configured sequence unattended (per-task container, gate, conditional push). Interactive confirmation remains the default for every mutating step; `--auto` prints the plan and proceeds.** | The orchestration is now deterministic and gated (D25) — the risk the manual rule guarded against (unreviewed agent-driven pushes) is closed structurally: only gate PASS reaches `origin/main`, and every step is logged in the run manifest. |
| D27 | Model + credentials (2026-08-28) | `ANTHROPIC_API_KEY` env; model/effort per D22 | **Versioned agent profiles in `experiment.toml`. Default profile `zai-flash`: claude CLI against the Z.ai Anthropic-compatible endpoint (`ANTHROPIC_BASE_URL`), model `glm-5.3-flash` pinned via `ANTHROPIC_DEFAULT_{OPUS,SONNET,HAIKU}_MODEL` + `--model`, effort `low` (`--effort` + `CLAUDE_CODE_EFFORT_LEVEL`). The token comes from 1Password via `op read` of the `op://` reference stored in the profile config; it is injected only through a 0600 `--env-file` (never argv, never logs, never artifacts) and every output path passes a redactor that knows the value.** | Reproduces the operator's `claude-zai-flash-yolo` setup (discovered, not guessed) inside the container, with the effort pinned (the host wrapper leaves effort to global settings — xhigh — so the container must pass it explicitly). |
| D28 | Task sequence (2026-08-28) | TASK-101..106 assuming a planner-built fixture repo | **TASK-001..007: 001 bootstraps the Cargo workspace from an empty `main`; 002-007 port 101-105-106 content (store+list, create form, connect+sidebar, preview+sort, custom SQL, disconnect/exit+gallery). Application code is never fixture-supplied: everything on `main` is agent-authored in some task. Each task package ships `trusted/` verification material (tests, support harness, seed, `render.rs`, font) overlaid at dispatch and committed as the trusted base commit; trusted paths are outside every whitelist. Trusted tests use behavioral assertions (contains/order/count, store roundtrips, exit codes), not golden snapshots.** | Empty-repo start (goal) + D25 scope base; planner-authored tests keep verification out of the agent's hands (E2/E3); golden snapshots require a maintained reference implementation per screen — behavioral assertions keep gates deterministic and honest while the snapshot ablation stays in the backlog (§7). |
| D29 | Gate config format | `verify.config` (bash, sourced) | **`verify.toml` (declarative, schema `verify/v1`); the gate is the `taskfmt verify` subcommand baked into the image. Scope base resolution: `--base` > `TASKFMT_BASE` env > `base_ref` in verify.toml > `baseline` tag. `EXTRA_CHECKS` bash hooks dropped.** | Config-as-code that the gate sources is executable surface area; data-only config cannot smuggle logic, parses identically on host and container, and the same Rust gate binary runs in both places (agent feedback loop and host verdict). |
| D30 | Preload artifact | `harness/images/preload/postgres.tar` committed (~109 MB) | **Tar gitignored; `postgres.digest` pin committed; `taskfmt preload` regenerates the tar (pull pinned digest → `docker save`).** | A 109 MB blob in git dwarfs the actual source; the digest pin keeps reproducibility, the CLI makes regeneration one command. |

### Superseded / retracted claims

- raw/01–02: "`/goal` evaluator ... GOAL_PROGRESS needed because evaluator reads transcript" — still true, but the critique's claim that headless has no evaluator is **wrong**; `/goal` runs in `-p`.
- raw/01: hooks "deterministically block task completion" via post-tool gates — **wrong**; only `PreToolUse` deny and `Stop` block, and Stop is capped.
- raw/02: whole design of mutable regions + normalizer — **replaced** by D1.
- raw/01: `.goal/progress/TASK-042.md` vs raw/02 `tasks/TASK-042/progress.md` path conflict — **resolved** by D14 (`/progress/progress.md`).
- raw/01: "instantiated tasks will be considerably shorter" — **false** for v2 (placeholders expand); true target now enforced by design (D2).

---

## 4. The v3 reference package

Location: `reference/task-template/` — pure task files, nothing else. Dispatch tooling (task-lint.sh, progress-init.sh, selftest.sh), the launch prompt (`harness/goal-prompt.md`) and the lint corpus (`harness/testdata/example/`) live in `harness/` (D15).

| File | Mount | Owner | Content |
| --- | --- | --- | --- |
| `README.md` | `/task` ro | planner | frontmatter (`id,title,kind,verify,expected_paths` — the scope whitelist, D23); Goal; Context (current/desired, read list, code flow, baseline command + expected failure); Preconditions with commands; Scope in/out; Requirements `R-*`; Acceptance table `AC-*`; Fixed decisions `D-*`; static numbered Checklist between `<!-- checklist:start/end -->`. |
| `AGENTS.md` (+ `CLAUDE.md` symlink) | `/task` ro | fixed | Goal line; one-paragraph layout (`/task` ro, `/progress`, `/work`); files table; 8-step protocol; `progress.md` grammar (fenced header, markdown-viewer readable); 6 prohibitions; BLOCKED/NEEDS_REPLAN; `GOAL_PROGRESS` turn line; final report grammar ending in `GOAL_RESULT`. |
| `verify.sh` | `/task` ro | fixed | Generic gate (D12). |
| `verify.config` | `/task` ro | planner | `BASE_REF`, command arrays, forbidden patterns/paths, required paths, allowed globs (scope whitelist), extra checks. |
| `progress.md` | `/progress` rw | agent | **Generated per run by `progress-init.sh` from `README.md`; never stored in the repo.** `TASK/STATE/CURRENT/BASELINE` header between `---` fences (renders in a markdown viewer); verbatim checklist copy; append-only `## Log` (`- <id> | DONE|FAILED|REOPENED|BLOCKED | <command -> result>`); `## Handoff` (`NEXT`, `CURRENT_FAILURE`, `DECISIONS`). |
| `task-lint.sh` | `harness/` | fixed | Author checklist as a program: frontmatter, sections, placeholders, `P-*` commands, `AC-*` rows, checklist grammar/contiguity/depth/leaf count/evidence/AC coverage/gate leaf, `verify.config` ↔ frontmatter consistency. |
| `progress-init.sh` | `harness/` | fixed | Lints, then emits the initial `progress.md`. Only way a `progress.md` comes into existence. |
| `selftest.sh` | `harness/` | fixed | Throwaway-fixture proof that lint rejects broken contracts and the gate fails on every tamper (`STATE`, `CURRENT`, `BASELINE`, `TASK`, parent/child, reword, delete, add) and passes only on the complete `DONE` file. |

Checklist rules (in `README.md` header + AGENTS.md): IDs `N`, `N.N`, `N.N.N`, `N.N.N.N`; exactly four spaces per level; depth = ID components; siblings contiguous from 1; leaf = item with no children; only leaves count; only a leaf may be `CURRENT`; parent `[x]` ⇔ all children `[x]`; 5–20 leaves; every leaf states what becomes true + evidence command; agent never edits text/IDs/order/indent; missing work → `NEEDS_REPLAN`.

Completion (host gate, only source of truth): `verify.sh` exit 0 AND last stdout line `DONE` on the final tree with trusted copies; implies scope respected (whitelist only, D23), all checks green, `progress.md` structurally consistent with `TASK:` = README `id`, `STATE: DONE`, `CURRENT: NONE`, `BASELINE:` recorded, all leaves `[x]`.

Why `progress.md` is generated, not stored (D1 corollary): a checked-in copy is a second source of truth for the checklist; the moment an author edits `README.md` the two drift and the gate's byte-diff fails for a reason unrelated to the agent. Generation at dispatch makes drift impossible and turns the generator into the single definition of the initial state.

---

## 5. Container harness — HEADED mode with herdr (decided; from notes/headed-herdr-harness.md)

Decision (operator mandate): agents run **headed** (interactive TUI) under **herdr** (https://herdr.dev/) in a **persistent** container (no `--rm`). The operator re-attaches later to watch the live session, scroll back, and inspect state with the agent still alive. tmux/zellij/screen are never used. The headless design in notes/container-harness.md is superseded except for image base, auth, mounts, network, and the host gate.

Verified (herdr 0.8.2, Apache-2.0, static Linux binary; tested in `debian:bookworm-slim` as non-root `agent`):

- Server: `herdr server` in the entrypoint (backgrounded; does not daemonize), `docker run -d` without `-t`. Socket `~/.config/herdr/sessions/<name>/herdr.sock`; control via `docker exec -u agent -e HERDR_SESSION=agent <c> herdr …`.
- Launch: `herdr workspace create --cwd /work --no-focus` → pane id; `herdr pane run <pane> "<cmd>"` where cmd = `HERDR_AGENT=claude script -qfec '<claude …>' /out/tui.log` (herdr has no pipe-to-file; `script` captures the raw stream; `HERDR_AGENT` makes herdr detect the agent behind the wrapper).
- Prompt injection: `herdr agent prompt task "<text>"` (bracketed-paste aware, presses Enter; refuses with `agent_blocked` if a dialog is open). Readiness: `herdr agent wait task --until idle`.
- Attach: `docker exec -it -u agent <c> herdr` (or `herdr agent attach task`); detach `ctrl+b q`. Read: `herdr pane read --source visible|detection`, `herdr pane wait-output --regex`. Quirk: `--source recent*` returned empty on headless Linux — avoid.
- Completion detection (`status.sh`): authoritative = transcript jsonl `attachment.type=="goal_status"` (`met`, `reason`, `sentinel`); then `GOAL_RESULT` in `/out/tui.log`; then `herdr agent wait --until idle|done|blocked`.
- Claude Code pre-seed to skip dialogs: `settings.json` `{skipDangerousModePermissionPrompt:true, tui:"default", theme, env:{CLAUDE_CODE_GOAL_CHECKIN_MINUTES:"0"}}`; `.claude.json` `hasCompletedOnboarding`, `projects["/work"].hasTrustDialogAccepted`; env `CLAUDE_CONFIG_DIR=/agent-home`, **`CLAUDE_CODE_DISABLE_ALTERNATE_SCREEN=1`** (else output lives on the alt screen, invisible to scrollback).
- `/goal` interactive = same Stop-hook evaluator; no `--max-turns`/budget caps headed → the turn bound in the condition text is the only cap. Goal survives `--resume`; bypass flag does not (pass again).
- Lifecycle: `docker stop` kills server + panes; entrypoint traps SIGTERM for graceful `herdr server stop`; restart relaunches `claude --resume <session-id> --dangerously-skip-permissions --add-dir …`; herdr `resume_agents_on_restore` disabled.
- Sketches in the note: Dockerfile deltas, entrypoint, `run-headed.sh`, `attach.sh`, `status.sh` (`--wait`, `--kill-after`), post-run inspection table, Codex TUI equivalents (`--no-alt-screen`, `trust_level="trusted"`).

Unchanged from the headless plan: `node:22-bookworm` base, non-root user, pinned CLI, telemetry off, API-only firewall with `NET_MODE=all` switch, mounts `/task` ro / `/work` rw / `/progress` rw / `/agent-home` / `/out`, fixture copy + `baseline` tag, host re-run of trusted `verify.sh`, `metrics.json`.

## 5b. Example fixture app — `pgtui` (decided; notes/example-app-{rust-stack,testing,decomposition}.md)

Purpose: real-world workload for comparing task formats. Rust TUI (ratatui) PostgreSQL browser: saved connections in local SQLite via Turso; create connection; connect → sidebar of tables + empty body; table preview `SELECT * FROM <t> LIMIT 500`; column sort asc/desc; custom SQL screen (no sort); disconnect/exit. Verification: unit tests, testcontainers integration (real PostgreSQL), insta text snapshots per screen (incl. `assert_debug_snapshot!(buffer)` for styles), visual SVG→PNG renders.

Stack (versions 2026-08-28): Rust 1.98 stable, edition 2024, MSRV 1.88; ratatui 0.30.2 + crossterm 0.29; tokio 1.53; `turso` 0.7.2 (`default-features = false`; "production-ready with caveats"); **tokio-postgres 0.7.18 simple-query protocol** (all cells text, `NULL` = None; chosen over sqlx 0.9 which would force MSRV 1.94 and `AssertSqlSafe` wrapping); clap 4.6; color-eyre 0.6.5; tui-input 0.15.4; testcontainers 0.28.0 + testcontainers-modules 0.15.0 (postgres `16-alpine`); insta 1.48.0; term rendering Buffer→SVG→resvg 0.48 with bundled DejaVu Sans Mono; cargo-nextest 0.9.143.

Decomposition: six linear tasks TASK-101→106 (one task fails every readiness rule: ~60 leaves, no incremental gate). 101 workspace + Turso store + connection-list screen; 102 create-connection; 103 connect + table sidebar (testcontainers); 104 preview grid + sort; 105 custom SQL; 106 disconnect/exit + gallery PNGs + snapshot completeness. ~70 fixed decisions D-* made up front (Elm-style `App::update(Msg) -> Vec<Effect>`, `Screen` enum, key bindings, store schema, DSN fields, sort semantics, LIMIT 500, 100×30 snapshot size, exit codes).

Protection model (whitelist only, D23, updated by D28): planner ships all trusted tests per task in `trusted/` (overlaid at dispatch, committed as the run-local trusted base commit, never pushed — D25); `expected_paths` for every task covers only what that task's agent may create or change (e.g. `crates/pgtui/src/*`), so `Cargo.toml`, `tests/*`, `render.rs`, fonts and every other trusted path sit outside the whitelist — any change to them fails the gate's scope check. Executor-written tests live in `src/` `#[cfg(test)]` only. Trusted tests are behavioral (no golden snapshots, D28). Baseline(N+1) = `origin/main` after task N; every gate must FAIL on the untouched pre-task state (the empty-repo start for TASK-001 makes this vacuous there: cargo itself is absent).

Risk: testcontainers from inside the agent container needs the Docker socket mounted (`/var/run/docker.sock`; testcontainers-rs has no Ryuk, cleanup on Drop) — UNVERIFIED under the herdr harness; P-* `docker info` precondition gives a clean BLOCKED.

Packages: `experiments/tasks/TASK-001..007/` (D28); seed `experiments/fixtures/seed/`.

## 6. Metrics per run (harness-computed, never from agent claims)

`gate_pass`; `false_done` (agent STATUS DONE ∧ ¬gate_pass); `false_blocked` (BLOCKED on a control task known satisfiable); `task_tamper` (`/task` write attempts in the tool log; the ro mount blocks them, the metric counts tries); `scope_violation` (changed files ∉ `expected_paths`, excluding `/progress`); `leaf_claim_accuracy` (harness re-runs each `[x]` leaf's evidence command); `state_consistency` (progress parses; ID set = README.md; DONE ⇔ all leaves); `report_conformance` (`GOAL_RESULT` last line, report grammar); `ac_coverage` (AC commands actually executed, from tool log); `verify_runs` (count, index of first); `turns`, `tool_calls`, tokens, `cost_usd`, `wall_s`, `compactions`; `diff_stability` across seeds (Jaccard of changed-file sets); `instruction_violations` (greps: `--no-verify`, `#[ignore]`, `skip(`, lint-suppression added, test files deleted).

Minimum design: ≥3 task kinds (bugfix, feature, removal) × ≥5 seeds × variant; report mean + stddev; a variant that raises `gate_pass` but raises `false_done` or scope variance is not more predictable.

---

## 7. Experiment backlog (one at a time, manual)

1. **v3 baseline** (this package) on Claude Code; then Codex.
2. Protocol-location ablation: same task, protocol in `AGENTS.md` vs inline in `README.md` vs in the launch prompt.
3. Checklist depth ablation: 4-level vs 2-level vs no checklist (hypothesis: <5-point `gate_pass` change for ≤15-leaf tasks; checklist earns its place on resume-after-compaction).
4. Rule-count ablation: 6 vs 15 vs 25 prohibitions.
5. Executable checklist: each leaf is a script under `/task/leaves/`; harness computes progress; agent writes no state (hypothesis: `false_done` → 0).
6. Harness-driven loop instead of `/goal`: one `claude -p`, host runs verify.sh, re-invoke with `--continue` + verifier output; identical for Codex.
7. Fresh reviewer pass (second `-p` with README.md + diff + verifier output → PASS/FAIL per R/AC).
8. Hooks: `PreToolUse` deny on `/task/*`, `SessionStart compact` re-injection — measure effect on `task_tamper` attempts and post-compaction resume.
9. ~~Task linter implementing the author checklist.~~ Done: `harness/task-lint.sh` (all six pgtui packages and the example pass; the six READMEs exceed the ~2,500-token target by 2–22%, reported as warnings — trim or accept as a measured variable).

---

## 8. Open / UNVERIFIED

- herdr: `docker exec -it … herdr` attach UX, `/goal …` via `agent prompt` (Enter vs slash popup), `customApiKeyResponses` format, Codex goal events. `/goal` under `--bare`; `SessionStart compact` injection reliability; Codex goal semantics in `exec`; Docker file-mount semantics on macOS; OpenAI harness-engineering quotes read via mirrors (primary 403).
- Instruction-file naming: `AGENTS.md` is the real file everywhere (repo root, task package, fixture); `CLAUDE.md` is always a symlink to it, so Claude Code and Codex read one file. Whether Claude Code auto-loads `/task/CLAUDE.md` via `--add-dir` in the headed run is UNVERIFIED (docs say added directories contribute their CLAUDE.md); Codex reads `AGENTS.md` only on the git-root→cwd chain, i.e. `/work`, so the launch prompt keeps naming `/task/AGENTS.md` explicitly.
- `check_progress` validates terminal state only; a `--partial` mid-run lint is a follow-up.
- Model dependence: Anthropic notes newer models may need less decomposition [A2]; task size is a tunable.

---

## Sources

- [A1] https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents
- [A2] https://www.anthropic.com/engineering/harness-design-long-running-apps
- [A3] https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
- [A4] https://claude.com/blog/building-agents-with-the-claude-agent-sdk
- [A5] https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents
- [C1] https://code.claude.com/docs/en/goal
- [C2] https://code.claude.com/docs/en/best-practices
- [C3] https://code.claude.com/docs/en/memory
- [C4] https://code.claude.com/docs/en/hooks , https://code.claude.com/docs/en/hooks-guide
- [C5] https://code.claude.com/docs/en/headless , https://code.claude.com/docs/en/cli-reference
- [C6] https://code.claude.com/docs/en/devcontainer , https://github.com/anthropics/claude-code/tree/main/.devcontainer
- [O1] https://developers.openai.com/cookbook/examples/codex/using_goals_in_codex , https://github.com/openai/codex/discussions/21764
- [O2] https://learn.chatgpt.com/use-cases/follow-goals
- [O3] https://developers.openai.com/cookbook/articles/codex_exec_plans
- [O4] https://openai.com/index/harness-engineering/ (via mirrors; UNVERIFIED)
- [O5] https://github.com/openai/symphony (SPEC.md)
- [O6] https://agents.md/ , https://learn.chatgpt.com/docs/agent-configuration/agents-md
- [O7] https://learn.chatgpt.com/docs/non-interactive-mode
- [E1] https://www.swebench.com/SWE-bench/guides/datasets/ , https://raw.githubusercontent.com/SWE-agent/SWE-agent/main/config/default.yaml
- [E2] https://www.harborframework.com/docs/tasks , https://arxiv.org/abs/2601.11868
- [E3] https://arxiv.org/abs/2604.28093 ; https://arxiv.org/abs/2607.08964
- [V1] https://docs.github.com/en/copilot/using-github-copilot/coding-agent/best-practices-for-using-copilot-to-work-on-tasks
- [V2] https://docs.devin.ai/essential-guidelines/instructing-devin-effectively
- [V3] https://ampcode.com/docs/prompting
- [G1] https://github.github.com/gfm/ §5.3
- Ralph Wiggum: https://github.com/ghuntley/how-to-ralph-wiggum , https://github.com/anthropics/claude-code/blob/main/plugins/ralph-wiggum/README.md
- Beads: https://github.com/steveyegge/beads
- aider JSON study: https://aider.chat/2024/08/14/code-in-json.html
