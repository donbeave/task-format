# Task harness (schema task/v3)

Everything required to author, dispatch, execute and gate a task package. The package itself is the research object and lives pure in `reference/task-template/`; this folder holds the tooling around it.

## Layout

```text
reference/task-template/    the task package — pure template, exactly what ships to the agent
  README.md                 contract: goal, context, preconditions, scope, R-*, AC table, D-*, static checklist
  AGENTS.md                 execution protocol (same for every task); progress grammar; final report
  CLAUDE.md -> AGENTS.md    symlink, so Claude Code auto-loads the protocol from /task (--add-dir) and Codex reads AGENTS.md
  verify.sh                 generic gate; reads verify.config next to it
  verify.config             project-specific commands, scope whitelist globs, patterns

harness/                    dispatch tooling — never shipped into /task
  task-lint.sh              author checklist below, mechanically
  progress-init.sh          README.md -> initial progress.md (lints first)
  selftest.sh               proves lint + progress-init + verify.sh on a throwaway fixture
  goal-prompt.md            the prompt used to start the agent (/goal condition for Claude Code, Codex variant)
  testdata/example/         lint corpus: a filled-in task (TASK-042) used by selftest.sh
  images/ + build.sh + preload.sh   container images: base/claude/codex, herdr, prereq stack
  run-headed.sh             dispatch one task to one agent TUI in a persistent container (below)
  attach.sh / status.sh     re-attach to the live agent; completion detection from outside

<run>/progress/progress.md  mounted read-write at /progress/progress.md; GENERATED per run, never stored
```

Scope is a whitelist only (D23): `expected_paths` in the README frontmatter equals `ALLOWED_GLOBS` in verify.config; every changed file outside it fails the gate. There is no blacklist and no per-file hash manifest — the read-only `/task` mount plus the whitelist cover the same ground with one list.

`progress.md` is derived state. It is generated from `README.md` by `progress-init.sh` at dispatch and lives only in the run directory (`.gitignore` blocks it everywhere). A stored copy would be a second source of truth that drifts from the checklist.

Copy the package with `cp -a` (keeps the symlink). Container mounts: `/task` (ro), `/work` (rw, fresh copy of the fixture repo with a `baseline` tag), `/progress` (rw).

## Dispatch

1. Write `README.md` from the template. Fill the checklist; every leaf has an evidence command.
2. Write `verify.config`. `BASE_REF="baseline"`; `ALLOWED_GLOBS` = `expected_paths` (whitelist).
3. `harness/task-lint.sh <task-dir>` must print `LINT PASS`.
4. `harness/progress-init.sh <task-dir> -o <run>/progress/progress.md` — fenced header (`TASK`, `STATE: IN_PROGRESS`, `CURRENT: <first leaf>`, `BASELINE: <not run>`) + verbatim checklist block + empty Log + Handoff. Refuses on lint failure.
5. Prove the gate: `verify.sh` must FAIL on the untouched fixture and PASS with a reference solution applied. A gate that cannot distinguish the two is not a gate.
6. Launch with `harness/goal-prompt.md`.

## Gate (host, after the container exits)

```sh
VERIFY_ROOT=<run>/workspace VERIFY_TASK_DIR=<run>/task-snapshot PROGRESS_FILE=<run>/progress.md \
  <run>/task-snapshot/verify.sh
```

Exit 0 and last line `DONE` is the only pass signal. The agent's own report is never load-bearing. The `progress` check requires: checklist block equal to `README.md` modulo `[ ]`/`[x]`, every leaf `[x]`, parents consistent, `TASK:` equal to the README `id`, `STATE: DONE`, `CURRENT: NONE`, `BASELINE:` recorded (not `<not run>`).

## Container harness (built)

Headed design (herdr) in `docs/research/notes/headed-herdr-harness.md`; image/mount/gate basics in `docs/research/notes/container-harness.md`; decisions D19-D22 in `docs/research/RESEARCH-FINDINGS.md` §3. Run outputs land in `experiments/runs/<ts>-<agent>-<task>/` (gitignored).

### Images

```text
harness/images/
  base/Dockerfile              node:22-bookworm, non-root `agent` user, git/jq, inner docker CLI
  claude/Dockerfile            base + pinned claude-code CLI + herdr
  codex/Dockerfile             base + pinned codex CLI + herdr
  common/entrypoint.sh         PID 1: prereq stage, then herdr server + agent pane
  common/prereqs.sh            inner dockerd (vfs) + docker load + prereq-postgres + seed restore
  common/agent-launch.sh       herdr workspace + script(1)-wrapped agent launch
  common/init-firewall.sh      API-only egress allowlist (deferred; v1 runs NET_MODE=all)
  common/herdr-config.toml     headless herdr server config (baked into the images)
```

- `harness/build.sh` builds `harness-base` and layers `harness-claude` / `harness-codex` on it.
- `harness/preload.sh` bakes the `postgres:16-alpine` image tarball into the images so the inner dockerd never needs the registry at run time.

### Per run (`harness/run-headed.sh`)

```sh
harness/run-headed.sh <claude|codex> <TASK-ID> [--model M] [--effort E] [--net api|all] [--seed-dir DIR]
```

Defaults: `--effort high`; `--model` `sonnet` (claude) / image default (codex); `--net all` (`api` documented for later — firewall deferred, D21); `--seed-dir ../experiments/fixtures/seed`. Steps:

1. Resolve the task dir and its fixture (named by the `fixture` file). An unbuilt fixture is normal today: the workspace starts empty with a `baseline` commit (smoke-run dispatch still valid). Task dir copied to `<run>/task-snapshot/`, topped up with `AGENTS.md`/`verify.sh` from `reference/task-template/` plus the `CLAUDE.md -> AGENTS.md` symlink.
2. `harness/task-lint.sh` must pass (`<run>/lint.log`; failure aborts dispatch). `harness/progress-init.sh` writes `<run>/progress/progress.md`. Workspace gets a `baseline` commit + tag.
3. Seed dir copied to `<run>/seed` and mounted `/seed:ro`; the default holds `pgtui-seed.sql`.
4. Prompt = the fenced `text` block of `harness/goal-prompt.md` collapsed to one line (`<run>/prompt.txt`). Claude runs with a fixed `--session-id`, so the transcript path is known before launch.
5. Agent home pre-seeded to skip dialogs: claude `settings.json` + `.claude.json`; codex `config.toml` (auth via `codex login` in the entrypoint).
6. `docker run -d --privileged` (inner dockerd for testcontainers, D19), named, **no `--rm`** — persistent by hard rule. Mounts `/work`, `/task:ro`, `/progress`, `/agent-home`, `/out` (+ `/seed:ro`); env `AGENT_CMD`, `AGENT_KIND`, `NET_MODE`, the API key, `CLAUDE_CONFIG_DIR`/`CODEX_HOME`. Model/effort per D22: claude `--model` + `--effort` + `CLAUDE_CODE_EFFORT_LEVEL`; codex `-m` (when set) + `-c model_reasoning_effort`.
7. Poll for `/out/prereqs.ready` (up to 180 s).
8. After ready: read `/out/pane-id`, write `meta.json` (run, container, agent, task, model, effort, net, session_id, pane, start), `herdr agent wait --until idle`, rename the agent to `task`, `herdr agent prompt task "$PROMPT"`, then confirm goal acceptance (claude: `goal_status` sentinel in the transcript; codex: agent turns `working`).
9. Print the summary block: run dir, container name, attach/status commands, raw log and transcript paths.

### Prereq stage (entrypoint, before the agent starts)

Inner `dockerd` (storage `vfs`) → `docker load` of the preloaded `postgres:16-alpine` → standing `prereq-postgres` (`pgtui/pgtui/pgtui` on `127.0.0.1:5432`) → seeds restored from `/work/**/tests/fixtures/seed.sql`, then `/seed/*.sql` → `/out/prereqs.ready` + `/out/prereqs.json`. On failure the entrypoint writes `/out/prereqs.FAILED` (+ `prereqs.json`, `prereqs.log`); `run-headed.sh` dumps both, prints the container name and exits 1, leaving the container up for inspection. Hermetic per-test testcontainers remain the canonical gate; the standing postgres only removes registry egress at run time and covers empty-fixture smoke runs.

### Attach / status

```sh
harness/attach.sh <RUN_ID> [--direct]    # docker exec -it ... herdr; detach ctrl+b q — never ctrl+c
harness/status.sh <RUN_ID> [--wait] [--kill-after MIN]
```

`status.sh` prints JSON state from three signals in order of trust: transcript `goal_status` verdict (claude), the `GOAL_RESULT` line in `/out/tui.log`, herdr's agent status (`idle`/`done` settled, `blocked` dialog, target gone = agent exited). It also snapshots the last rendered screen to `<run>/out/screen.txt`.

## Author checklist (enforced by `task-lint.sh`)

- One outcome, one repo, one subsystem, no open decisions, no dependency on prior chat.
- Every `P-*` has a command. Every `AC-*` row has an evidence command and expected result.
- Checklist: IDs unique and contiguous; depth matches indentation (0/4/8/12 spaces = 1-4 components); max depth 4; 5-20 leaves; every leaf has evidence; no single-child levels added just for depth; every `AC-*` referenced; last leaf is the `verify.sh` gate.
- `expected_paths` (scope whitelist) covers what the solution may change or create and equals `ALLOWED_GLOBS`; no other path list exists.
- `verify.sh` fails on baseline, passes on the reference solution (not lintable; step 6).
- `README.md` under ~2,500 tokens after placeholders are replaced (lint warns above 10,000 bytes).

`selftest.sh` exercises the tools end to end (lint accept/reject, generated progress shape, gate FAIL on fresh/tampered progress, PASS only on the complete DONE file). Run it after touching any script here.
