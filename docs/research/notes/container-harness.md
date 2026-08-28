# Container harness — one agent, one task, one container

Status: research note, 2026-08-28. Verified against live docs + local binaries
(`claude` 2.1.250, `codex-cli` 0.150.1). Items marked **UNVERIFIED** were not
confirmed by an official page; test before relying on them.

Scope: manual, one-at-a-time runs. No orchestration. Goal is to hand
`task.md + verify.sh + progress.md` (see raw/01, raw/02) to a fresh agent in a
fresh container with a single prompt, then capture everything for manual
analysis.

---

## 1. Claude Code headless — verified

Source: https://code.claude.com/docs/en/cli-reference, /docs/en/headless,
/docs/en/goal, /docs/en/hooks, /docs/en/authentication, /docs/en/devcontainer,
/docs/en/network-config.

| Item | Status | Notes |
|---|---|---|
| `claude -p "<prompt>"` | verified | exits 0 on success, non-zero on failure; failures inside the run (e.g. missing auth) print as the `result` on stdout, so check `.is_error`/`.subtype` in JSON too |
| `--output-format stream-json --verbose` | verified | NDJSON; first event `system/init`, last line `result` with `total_cost_usd`, `num_turns`, `duration_ms`, `session_id`, `usage`. `--include-partial-messages` adds token deltas (noise for our purpose; skip). `--include-hook-events` adds hook lifecycle |
| `--output-format json` | verified | single object, `.result`, `.session_id`, `.total_cost_usd`, per-model cost breakdown |
| `--allowedTools "Bash,Read,Edit,Write"` | verified | permission-rule syntax; `Bash(git diff *)` prefix form |
| `--dangerously-skip-permissions` | verified | = `--permission-mode bypassPermissions`. **Rejected when running as root** → container must run as non-root user |
| `--permission-mode dontAsk` | verified | denies anything not in allow rules; alternative to bypass if we want a hard tool allowlist instead |
| `--max-turns N` | verified | print mode only; exits with error at limit |
| `--max-budget-usd X` | verified | print mode only; includes subagents |
| `--model sonnet|opus|haiku|fable|<full-id>` | verified | |
| `--session-id <uuid>` | verified | lets us pick transcript filename up front |
| `--no-session-persistence` | verified | do NOT use — we want the transcript |
| `--bare` | verified | skips hooks/skills/plugins/CLAUDE.md/MCP/auto-memory; Anthropic says will become default for `-p`. **Does not read `CLAUDE_CODE_OAUTH_TOKEN` or keychain** — API key only. Since `/goal` is implemented as a hooks-system feature and `--bare` skips hooks, `/goal` under `--bare` is **UNVERIFIED (likely unavailable)** — test once |
| `--append-system-prompt` / `--append-system-prompt-file` | verified | useful for injecting harness rules without touching task.md |
| `--add-dir <path>` | verified | grants read/edit to extra dirs; does not load `.claude/` from them |
| `--settings <file-or-json>` | verified | pin hooks/permissions per run, overrides settings.json keys |

### `/goal` in `-p` mode — verified: WORKS

Contrary to the assumption in the task brief, the official /goal page now has a
"Run non-interactively" section:

```bash
claude -p "/goal CHANGELOG.md has an entry for every PR merged this week"
```

Facts that matter for the harness:

- `/goal` is a session-scoped **prompt-based Stop hook**. After every turn a
  small model (Haiku by default; `ANTHROPIC_DEFAULT_HAIKU_MODEL` to change)
  judges the condition against **the conversation only** — it runs no commands,
  reads no files. Hence task.md's "surface verify.sh output in the transcript"
  rule is load-bearing.
- Condition max 4,000 chars. Put a turn cap in the condition text
  (`... or stop after 20 turns`) and/or pass `--max-turns`.
- Loop stops on: met / judged impossible / `/goal clear` / auth failure,
  credit exhaustion, unrecoverable context overflow, unavailable model. Also
  stops (goal left set, control returned = process exits in `-p`) if several
  consecutive turns make no tool calls.
- Text output prints nothing until the end; use `stream-json --verbose` to
  watch.
- Requires workspace trust rules same as hooks and `disableAllHooks != true`.
  In `-p` there is no trust dialog; project `.claude/settings.json` hooks run
  unless `--bare`. So the fixture repo's `.claude/` is executed — treat it as
  part of the experiment input (or strip it).
- Background bash / subagent still running at turn end → evaluation deferred;
  check-ins every 30 min (`CLAUDE_CODE_GOAL_CHECKIN_MINUTES=0` disables).

Alternatives if we ever need deterministic control instead of the model
evaluator: a command `Stop` hook in `--settings` that runs `verify.sh` and
returns `decision: block` until it prints DONE (Stop hook input has
`stop_hook_active`, `last_assistant_message`, `transcript_path`; blocking works
in `-p` but a hook that never releases hangs the process — pair with
`--max-turns`). Or Agent SDK. Not needed for v1.

### Auth inside Docker — verified

Precedence (headless): `CLAUDE_CODE_USE_BEDROCK/VERTEX/FOUNDRY` >
`ANTHROPIC_AUTH_TOKEN` > `ANTHROPIC_API_KEY` (always used in `-p`, no approval
prompt) > `apiKeyHelper` > `CLAUDE_CODE_OAUTH_TOKEN` (from `claude setup-token`,
1-year, Pro/Max/Team/Enterprise; model requests only) > `/login` creds.

- Simplest: `-e ANTHROPIC_API_KEY` (Console billing).
- Subscription billing: `claude setup-token` on host once, pass
  `-e CLAUDE_CODE_OAUTH_TOKEN`. Not read under `--bare`.
- Never bind-mount host `~/.claude` (contains `.credentials.json`, all
  transcripts). Use a throwaway `CLAUDE_CONFIG_DIR` inside the container.
- Linux creds land at `$CLAUDE_CONFIG_DIR/.credentials.json` (0600) if login
  ever happens; with env-var auth nothing is written.

### Transcript location — verified

`~/.claude/projects/<cwd-with-slashes-replaced-by-dashes>/<session-id>.jsonl`
(e.g. cwd `/work` → `~/.claude/projects/-work/<uuid>.jsonl`; this machine shows
`-Users-donbeave-Projects-donbeave-task-format/<uuid>.jsonl`). Subagent
transcripts sit in a sibling `<session-id>/` dir. `CLAUDE_CONFIG_DIR` relocates
the whole tree. With `--session-id` we know the path before the run. Stop-hook
docs warn the file is written asynchronously and can lag — copy it only after
the process exits.

### Official container reference — verified

`github.com/anthropics/claude-code/.devcontainer/{Dockerfile,devcontainer.json,init-firewall.sh}`:

- `FROM node:20`, apt: git, procps, sudo, zsh, gh, iptables, ipset, iproute2,
  dnsutils, aggregate, jq, …; user `node`; `npm install -g
  @anthropic-ai/claude-code@${CLAUDE_CODE_VERSION}`; `DISABLE_AUTOUPDATER=1`
  recommended for pinning.
- `init-firewall.sh`: default-DROP iptables + ipset `allowed-domains`
  containing resolved IPs of `registry.npmjs.org api.anthropic.com sentry.io
  statsig.com marketplace.visualstudio.com vscode.blob.core.windows.net
  update.code.visualstudio.com` + GitHub CIDRs (api.github.com/meta) + host
  subnet + DNS:53 + SSH:22. Needs `--cap-add NET_ADMIN --cap-add NET_RAW`.
  Ends with negative (example.com) / positive (api.github.com/zen) checks.
- Docs explicitly bless `--dangerously-skip-permissions` inside a container
  with a non-root user, paired with egress restriction.

Required egress for our use (network-config page): `api.anthropic.com` only
for API-key auth. Optional/telemetry: `http-intake.logs.us5.datadoghq.com`,
`browser-intake-us5-datadoghq.com` (older firewall script lists sentry/statsig)
— kill with `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, `DISABLE_TELEMETRY=1`,
`DISABLE_ERROR_REPORTING=1`. OAuth token refresh needs `platform.claude.com`.

---

## 2. Codex CLI — verified

Sources: https://learn.chatgpt.com/docs/non-interactive-mode,
/docs/developer-commands?surface=cli, /docs/agent-approvals-security,
/docs/config-file/config-reference.md, /docs/agent-configuration/agents-md.md,
github.com/openai/codex/discussions/21764; local `codex exec --help` (0.150.1).

| Item | Status | Notes |
|---|---|---|
| `codex exec "<prompt>"` / `codex exec -` (stdin) | verified | progress → stderr, final message → stdout; piped stdin with a prompt arg is appended as `<stdin>` block |
| `--json` | verified | JSONL events on stdout (thread started, turns, items, errors) |
| `-o, --output-last-message <file>` | verified | final agent message to file |
| `--output-schema <file>` | verified | structured final answer |
| `-s, --sandbox read-only\|workspace-write\|danger-full-access` | verified | default in exec is `read-only` (docs) |
| `--full-auto` | verified | **deprecated**; use `--sandbox workspace-write` |
| `--dangerously-bypass-approvals-and-sandbox` (`--yolo`) | verified | official guidance for Docker: if bwrap/seccomp/namespaces unavailable in the container, "configure Docker to provide isolation, then run with this flag" |
| `-a, --ask-for-approval untrusted\|on-request\|never` | verified (docs page) | `--help` in 0.150.1 shows `--approve-for-me` instead; set `-c approval_policy=never` to be explicit |
| `-C, --cd <dir>`, `--add-dir`, `--skip-git-repo-check` | verified | |
| `--ephemeral` | verified | do NOT use — we want rollout files |
| `--ignore-user-config` | verified | skip `$CODEX_HOME/config.toml`; auth still from `CODEX_HOME` |
| `-c key=value` | verified | TOML-typed overrides, e.g. `-c sandbox_workspace_write.network_access=true`, `-c model_reasoning_effort="high"` |
| `-m, --model` | verified | |

### Auth — verified

- Recommended for automation: `CODEX_API_KEY=<key> codex exec ...` (docs say
  set per-invocation, not as persistent job env).
- Alternative: `printenv OPENAI_API_KEY | codex login --with-api-key` writes
  `$CODEX_HOME/auth.json`. ChatGPT-plan OAuth requires a browser login on host
  and copying `auth.json` in — avoid for the harness.
- `CODEX_HOME` (default `~/.codex`) holds `config.toml`, `auth.json`,
  `sessions/`, `log/`, `state_*.sqlite`.

### Session capture — verified (community + docs)

Rollouts: `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-<ts>-<id>.jsonl` — full
trace incl. tool calls. Logs: `$CODEX_HOME/log`. Point `CODEX_HOME` at a
per-run mounted dir and copy the whole thing.

### AGENTS.md discovery — verified

Global `~/.codex/AGENTS.override.md` → `~/.codex/AGENTS.md` (first non-empty),
then project root (git root) walking down to cwd, per dir
`AGENTS.override.md` → `AGENTS.md` → `project_doc_fallback_filenames`.
Concatenated root→cwd, budget `project_doc_max_bytes` = 32 KiB. No git root →
cwd only. So the fixture repo's `AGENTS.md` is read; task dir mounted outside
the repo is not. Keep it that way (task.md is passed by prompt, not by
instruction file).

### Goals in `codex exec` — UNVERIFIED / experimental

- `/goal` exists in TUI/app/IDE (0.128+), `features.goals` default `true` in
  current config reference; `/goal pause|resume|clear|edit`; objective ≤ 4,000
  chars; loop stops on success/pause/clear/interrupt/budget/blocker; a
  continuation turn with no tool call suppresses the next continuation.
- `codex exec --help` (0.150.1) has **no `--goal` flag**. Maintainer
  (2026-05-09, discussion #21764): "not a first-class command... experimental."
  Workaround reported: prompt "Create a goal for this thread using the goal
  tool, not as prose. After creating it, call get_goal." — then it continues.
- Harness decision: for Codex v1, do NOT rely on goals. Use a plain
  `codex exec` with the task prompt and treat one exec = one attempt; if it
  exits before DONE, optionally `codex exec resume --last "continue until
  ./verify.sh prints DONE"` up to N times from `run.sh`. Try the goal-tool
  prompt as a separate experiment variant.

### Codex sandbox in Docker — verified

Linux sandbox = bubblewrap + seccomp. Inside Docker it often fails
(namespaces/setuid). Two options: (a) `--cap-add SYS_ADMIN`-ish privileges so
bwrap works (Codex devcontainer reference does this) — fragile; (b) let Docker
be the sandbox and pass `--dangerously-bypass-approvals-and-sandbox`. Choose
(b); it mirrors Claude's `--dangerously-skip-permissions` so both agents get
the same isolation model (container + read-only mounts + network policy).

---

## 3. Mount strategy

Principle from raw/01: verifier must sit outside executor authority. Docker
gives that for free — the executor can't escape a `:ro` bind mount, and the
trusted gate runs on the host after the container exits.

```
container view                       host source                      mode
/work            (cwd of agent)      runs/<id>/workspace/             rw   fresh copy of fixture repo (git init'd)
/task            (task.md, ...)      experiments/tasks/<task-id>/     ro   contract; copied into runs/<id>/task-snapshot too
/task/progress.md                    runs/<id>/progress.md            rw   single-file bind on top of the ro dir (see note)
/home/agent/.claude or /codex-home   runs/<id>/agent-home/            rw   CLAUDE_CONFIG_DIR / CODEX_HOME → transcripts land here
/out                                 runs/<id>/out/                   rw   agent may drop logs here; run.sh also writes stdout here
(nothing)        verify.sh           experiments/tasks/<id>/verify.sh —    executed by host after exit; also present in /task:ro for the agent's own loop
```

Notes:

- Docker allows a file bind mount over a path inside a read-only dir mount
  (`-v .../task:/task:ro -v .../progress.md:/task/progress.md`). Order the `-v`
  flags outer first. Alternative if that proves flaky: mount progress at
  `/progress/progress.md` and reference that path in task.md frontmatter
  (`progress_log`).
- Checklist state lives inside task.md (raw/02) → task.md cannot be `:ro`
  wholesale. Two choices: (1) mount a **copy** of task.md rw at
  `/task/task.md`, keep the pristine original on host, and let the outer gate
  diff with the mutable-region normalizer from raw/02; (2) keep task.md ro and
  move checkbox state to progress.md. (1) matches the current template; do
  that. verify.sh stays ro.
- The agent runs `./verify.sh` from `/work`; put a tiny `/work/verify.sh`
  wrapper `exec /task/verify.sh "$@"` in the fixture (or reference `/task/
  verify.sh` in the prompt). The host re-runs the pristine copy anyway.
- Workspace is a fresh `cp -a` of the fixture, `git init && git add -A && git
  commit -m baseline` so `git diff baseline` captures exactly what the agent
  did, including untracked files via `git status --porcelain`.
- Strip or freeze the fixture's `.claude/` and `AGENTS.md`/`CLAUDE.md`: they
  are inputs the agent executes/reads. Hash them with the protected set.

## 4. Network

| Mode | How | Pros | Cons |
|---|---|---|---|
| allow-all | default bridge | zero setup; npm/pip/cargo fetches work; matches "real" dev | exfil possible with bypass-permissions; run-to-run drift from registry state |
| API-only | `--cap-add NET_ADMIN --cap-add NET_RAW` + init-firewall.sh (Claude reference) with allowlist `api.anthropic.com` (+ `platform.claude.com` for OAuth) / `api.openai.com` (+ `chatgpt.com` for OAuth) | deterministic; contains blast radius | any task needing dependency install must be pre-baked into the fixture image; firewall script needs root at start then drops to user |
| none | `--network none` | — | agent can't reach API; only viable with a local proxy on a sidecar; skip |

Decision for v1: **API-only** via allowlist, with a `NET_MODE=all` switch in
`run.sh` for tasks whose fixture needs installs. Record the mode in
`metrics.json`. Codex has a built-in `features.network_proxy` with domain
allowlist, but it only applies to sandboxed commands, which we bypass — so use
the iptables approach for both agents.

---

## 5. Directory layout — `experiments/`

```
experiments/
  README.md
  images/
    claude/Dockerfile
    codex/Dockerfile
    common/init-firewall.sh          # copied from anthropics/claude-code, domains parameterised
    common/entrypoint.sh             # applies firewall as root, then `exec gosu agent "$@"`
  fixtures/
    <fixture-name>/                  # a small repo the task acts on (committed, no .git)
      AGENTS.md / CLAUDE.md          # optional; part of the experiment input
      ...
  tasks/
    <TASK-ID>/
      task.md                        # goal-task/v2 contract (raw/02 template)
      verify.sh                      # trusted gate, exec bit set
      progress.md                    # empty template
      protected.txt                  # newline list of container paths hashed before/after
      fixture                        # one line: fixture dir name
      prompt.txt                     # optional override of the default prompt
  run.sh                             # entry: run.sh <agent> <TASK-ID> [--model X] [--net all|api]
  lib/
    gate.sh                          # host-side verify + hash + normalized task.md diff
    normalize-task.py                # strips checkbox tokens + status values (raw/02) before hashing
  runs/
    <YYYYMMDD-HHMMSS>-<agent>-<TASK-ID>/
      meta.json                      # agent, version, model, flags, image digest, net mode, start/end
      task-snapshot/                 # exact task dir used (task.md pristine copy)
      workspace/                     # post-run workspace (baseline commit inside)
      agent-home/                    # CLAUDE_CONFIG_DIR or CODEX_HOME contents
      transcript.jsonl               # copied from agent-home (Claude session / Codex rollout)
      stdout.ndjson                  # stream-json (Claude) / --json (Codex)
      stderr.log
      last-message.txt               # Codex -o / Claude .result
      diff.patch                     # git diff baseline..HEAD + untracked
      files-touched.txt
      hashes-before.txt / hashes-after.txt / hashes.diff
      verify-host.log                # trusted re-run output, last line, exit code
      metrics.json
```

## 6. Dockerfile sketches

### Claude Code

```dockerfile
# experiments/images/claude/Dockerfile
FROM node:22-bookworm
ARG CLAUDE_CODE_VERSION=2.1.250
RUN apt-get update && apt-get install -y --no-install-recommends \
      git jq ripgrep iptables ipset iproute2 dnsutils aggregate gosu ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN useradd -m -s /bin/bash agent
ENV NPM_CONFIG_PREFIX=/usr/local/share/npm-global
ENV PATH=$NPM_CONFIG_PREFIX/bin:$PATH
RUN npm install -g @anthropic-ai/claude-code@${CLAUDE_CODE_VERSION}
ENV DISABLE_AUTOUPDATER=1 \
    CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 \
    DISABLE_TELEMETRY=1 DISABLE_ERROR_REPORTING=1 \
    CLAUDE_CONFIG_DIR=/agent-home
COPY common/init-firewall.sh /usr/local/bin/init-firewall.sh
COPY common/entrypoint.sh    /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/*.sh && mkdir -p /agent-home /work /out && chown agent /agent-home /work /out
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]   # root: firewall (if NET_MODE=api) → gosu agent
```

### Codex

```dockerfile
# experiments/images/codex/Dockerfile
FROM node:22-bookworm
ARG CODEX_VERSION=0.150.1
RUN apt-get update && apt-get install -y --no-install-recommends \
      git jq ripgrep iptables ipset iproute2 dnsutils aggregate gosu ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN useradd -m -s /bin/bash agent
RUN npm install -g @openai/codex@${CODEX_VERSION}
ENV CODEX_HOME=/agent-home
COPY common/init-firewall.sh /usr/local/bin/init-firewall.sh
COPY common/entrypoint.sh    /usr/local/bin/entrypoint.sh
COPY codex/config.toml       /etc/codex-config.toml   # copied into $CODEX_HOME at entry
RUN chmod +x /usr/local/bin/*.sh && mkdir -p /agent-home /work /out && chown agent /agent-home /work /out
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
```

`codex/config.toml` (minimal, explicit):

```toml
approval_policy = "never"
sandbox_mode    = "danger-full-access"   # Docker is the sandbox
[features]
goals = true
```

`common/entrypoint.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail
if [ "${NET_MODE:-api}" = "api" ]; then
  ALLOWED_DOMAINS="${ALLOWED_DOMAINS:-api.anthropic.com}" /usr/local/bin/init-firewall.sh
fi
[ -f /etc/codex-config.toml ] && [ ! -f "$CODEX_HOME/config.toml" ] && cp /etc/codex-config.toml "$CODEX_HOME/config.toml" && chown agent "$CODEX_HOME/config.toml"
exec gosu agent "$@"
```

Fixture toolchains (python, cargo, …) go into a per-fixture image layered on
top (`FROM harness-claude:latest` + `RUN apt-get install ...`), so the agent
never needs network for dependencies under `NET_MODE=api`.

## 7. `run.sh` sketch

```bash
#!/usr/bin/env bash
# usage: run.sh <claude|codex> <TASK-ID> [--model M] [--net api|all] [--max-turns N] [--budget USD]
set -euo pipefail
AGENT=$1; TASK=$2; shift 2
MODEL=""; NET_MODE=api; MAX_TURNS=40; BUDGET=10
while [ $# -gt 0 ]; do case $1 in
  --model) MODEL=$2; shift 2;; --net) NET_MODE=$2; shift 2;;
  --max-turns) MAX_TURNS=$2; shift 2;; --budget) BUDGET=$2; shift 2;; *) echo "bad arg $1"; exit 2;; esac; done

ROOT=$(cd "$(dirname "$0")" && pwd)
TASK_DIR=$ROOT/tasks/$TASK
FIXTURE=$ROOT/fixtures/$(cat "$TASK_DIR/fixture")
RUN_ID=$(date -u +%Y%m%d-%H%M%S)-$AGENT-$TASK
RUN=$ROOT/runs/$RUN_ID
mkdir -p "$RUN"/{workspace,agent-home,out,task-snapshot}

# 1. fresh inputs
cp -a "$TASK_DIR"/. "$RUN/task-snapshot/"                 # pristine, never mounted
cp -a "$FIXTURE"/. "$RUN/workspace/"
cp "$TASK_DIR/task.md"     "$RUN/task.md"                 # mutable copy mounted rw
"$ROOT/../reference/task-template/progress-init.sh" "$TASK_DIR" -o "$RUN/progress.md"   # generated, never stored
( cd "$RUN/workspace" && git init -q && git add -A && git commit -qm baseline --no-verify && git tag baseline )

# 2. hashes before (protected.txt lists container paths; map /task→snapshot, /work→workspace)
"$ROOT/lib/hash-protected.sh" "$RUN" before > "$RUN/hashes-before.txt"

# 3. prompt
PROMPT=$(cat "$TASK_DIR/prompt.txt" 2>/dev/null || true)
: "${PROMPT:="Implement exactly the single task in /task/task.md. Read it first and restate the task ID, one-sentence goal and acceptance criteria. Update only the marked execution-status values, checkbox tokens and /task/progress.md. Do not modify /task/verify.sh or any protected path. Work until /task/verify.sh exits 0 and its final stdout line is exactly DONE; show the full verifier output in your final message."}"
SESSION_ID=$(uuidgen | tr A-Z a-z)

# 4. docker common args
CAPS=(); [ "$NET_MODE" = api ] && CAPS=(--cap-add NET_ADMIN --cap-add NET_RAW)
COMMON=(docker run --rm --name "$RUN_ID" "${CAPS[@]}" \
  -e NET_MODE="$NET_MODE" \
  -v "$RUN/workspace:/work" \
  -v "$TASK_DIR:/task:ro" \
  -v "$RUN/task.md:/task/task.md" \
  -v "$RUN/progress.md:/task/progress.md" \
  -v "$RUN/agent-home:/agent-home" \
  -v "$RUN/out:/out" \
  --memory 4g --cpus 2 --pids-limit 2048)

START=$(date -u +%s)
case $AGENT in
  claude)
    "${COMMON[@]}" -e ANTHROPIC_API_KEY -e ALLOWED_DOMAINS=api.anthropic.com harness-claude:latest \
      claude -p "/goal $PROMPT" \
        --output-format stream-json --verbose \
        --dangerously-skip-permissions \
        --session-id "$SESSION_ID" \
        --max-turns "$MAX_TURNS" --max-budget-usd "$BUDGET" \
        ${MODEL:+--model "$MODEL"} \
        >"$RUN/stdout.ndjson" 2>"$RUN/stderr.log" || echo "$?" >"$RUN/exit-code"
    ;;
  codex)
    "${COMMON[@]}" -e CODEX_API_KEY -e ALLOWED_DOMAINS=api.openai.com harness-codex:latest \
      codex exec --json --skip-git-repo-check -C /work \
        --dangerously-bypass-approvals-and-sandbox \
        --add-dir /task \
        -o /out/last-message.txt \
        ${MODEL:+-m "$MODEL"} \
        "$PROMPT" \
        >"$RUN/stdout.ndjson" 2>"$RUN/stderr.log" || echo "$?" >"$RUN/exit-code"
    ;;
esac
END=$(date -u +%s); [ -f "$RUN/exit-code" ] || echo 0 >"$RUN/exit-code"

# 5. collect artefacts (container is gone; everything is on host bind mounts)
case $AGENT in
  claude) cp "$RUN/agent-home/projects/-work/$SESSION_ID.jsonl" "$RUN/transcript.jsonl" 2>/dev/null || true
          jq -r 'select(.type=="result") | .result' "$RUN/stdout.ndjson" >"$RUN/last-message.txt" || true;;
  codex)  find "$RUN/agent-home/sessions" -name 'rollout-*.jsonl' -exec cp {} "$RUN/transcript.jsonl" \; 2>/dev/null || true
          cp "$RUN/out/last-message.txt" "$RUN/" 2>/dev/null || true;;
esac
( cd "$RUN/workspace"
  git add -A -N . 2>/dev/null; git diff baseline > "$RUN/diff.patch"
  git status --porcelain > "$RUN/files-touched.txt" )

# 6. trusted gate: hashes, normalized task.md diff, independent verify.sh run
"$ROOT/lib/hash-protected.sh" "$RUN" after > "$RUN/hashes-after.txt"
diff "$RUN/hashes-before.txt" "$RUN/hashes-after.txt" > "$RUN/hashes.diff" && PROTECTED_OK=1 || PROTECTED_OK=0
python3 "$ROOT/lib/normalize-task.py" "$RUN/task-snapshot/task.md" "$RUN/task.md" && TASK_OK=1 || TASK_OK=0
# verifier runs from the PRISTINE copy in a throwaway container, no network, same image
docker run --rm --network none -v "$RUN/workspace:/work" -v "$RUN/task-snapshot:/task:ro" \
  -w /work harness-$AGENT:latest bash -c '/task/verify.sh' >"$RUN/verify-host.log" 2>&1; VERIFY_RC=$?
VERIFY_LAST=$(tail -n1 "$RUN/verify-host.log")

# 7. metrics
jq -n --arg agent "$AGENT" --arg task "$TASK" --arg model "$MODEL" --arg net "$NET_MODE" \
  --argjson start "$START" --argjson end "$END" --argjson rc "$(cat "$RUN/exit-code")" \
  --argjson verify_rc "$VERIFY_RC" --arg verify_last "$VERIFY_LAST" \
  --argjson protected_ok "$PROTECTED_OK" --argjson task_ok "$TASK_OK" \
  --argjson files "$(wc -l < "$RUN/files-touched.txt")" \
  --argjson diff_lines "$(grep -c '^[+-]' "$RUN/diff.patch" || echo 0)" \
  '{agent:$agent,task:$task,model:$model,net:$net,wall_s:($end-$start),agent_exit:$rc,
    verify_exit:$verify_rc,verify_last_line:$verify_last,done:($verify_rc==0 and $verify_last=="DONE"),
    protected_unchanged:($protected_ok==1),task_static_unchanged:($task_ok==1),
    files_touched:$files,diff_lines:$diff_lines}' > "$RUN/metrics.json"
# Claude: merge cost/turns from result event
[ "$AGENT" = claude ] && jq -s '.[0] + (.[1] | select(.type=="result") | {cost_usd:.total_cost_usd,turns:.num_turns,duration_api_ms:.duration_api_ms,usage:.usage,stop_subtype:.subtype})' \
  "$RUN/metrics.json" "$RUN/stdout.ndjson" > "$RUN/metrics.tmp" && mv "$RUN/metrics.tmp" "$RUN/metrics.json"
echo "$RUN"; cat "$RUN/metrics.json"
```

`lib/hash-protected.sh` reads `tasks/<id>/protected.txt` (container paths such
as `/task/verify.sh`, `/work/.claude/settings.json`, `/work/AGENTS.md`), maps
`/task/*` → `task-snapshot/` for "before" and → the mounted `runs/<id>/task*`
files for "after" (only task.md/progress.md are actually mutable; verify.sh is
ro so its hash trivially matches — keep it anyway as a tamper tripwire), and
prints `sha256  path` lines. `lib/normalize-task.py` implements raw/02 §
"Protecting the task contract": replace `[x]`→`[ ]`, blank the values inside
the `goal-task:...:start/end` status markers, then byte-compare.

Open items to test on first run: (a) file bind over `:ro` dir mount ordering;
(b) whether `/goal` survives without `--bare` when the fixture has no
`.claude/`; (c) Codex rollout path under `CODEX_HOME=/agent-home`; (d)
init-firewall.sh needs `/etc/resolv.conf` DNS reachable before ipset build.

## 8. Cheap per-run metrics

From artefacts already captured, no extra instrumentation:

- Outcome: host `verify.sh` exit code + last line (`done` bool); agent exit
  code; Claude `result.subtype` (`success` / `error_max_turns` /
  `error_max_budget` …); protected hashes unchanged; normalized task.md
  unchanged.
- Cost/effort: Claude `total_cost_usd`, `num_turns`, `duration_ms`,
  `duration_api_ms`, `usage.{input,output,cache_read,cache_creation}_tokens`,
  per-model breakdown; Codex `--json` `turn.completed` usage events (sum);
  wall-clock seconds.
- Tool behaviour (from transcript / stdout.ndjson): count of tool_use by name;
  number of `Bash` calls whose command contains `verify.sh`; index of first
  and last `verify.sh` invocation; number of goal-evaluator verdicts and
  their reasons (Claude: hook events / transcript `goal` entries); did agent
  edit task.md outside mutable regions (from normalize diff); did agent touch
  `/out`.
- Change size: `files_touched`, `diff_lines`, files added vs modified vs
  deleted (`git status --porcelain` prefixes), any path outside task.md
  "Expected touch points" (grep frontmatter list vs files-touched).
- Checklist: leaf checkbox count checked vs total in final task.md (raw/02
  leaf-only rule), number of `progress.md` entries, whether final message
  contains the verifier output (regex on `last-message.txt`).
- Environment: image digest, CLI version (`claude --version` /
  `codex --version` captured in meta.json at run start), model, net mode.
