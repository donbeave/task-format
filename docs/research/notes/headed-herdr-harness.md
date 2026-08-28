> Superseded in part by D23 (2026-08-28): `lib/hash-protected.sh`, `protected.sha256`, the manifest step described below no longer exist — scope is the `expected_paths` whitelist alone (`RESEARCH-FINDINGS.md` §3 D23, D26). Body left as written; D24–D32 changes are in `synthesis-2026-08-28.md`.

# Headed harness — agent TUI inside Docker under herdr

Status: research note, 2026-08-28. Complements `container-harness.md` (headless
`claude -p` / `codex exec`). Multiplexer is **herdr** (https://herdr.dev) by
mandate; nothing else is considered. Verified against: herdr 0.8.2 docs
(https://herdr.dev/llms.txt → raw `docs/next/website/src/content/docs/*.mdx` at tag
`v0.8.2`), local `herdr 0.8.2` (`brew`), the static `herdr-linux-aarch64` release
binary run inside `debian:bookworm-slim` containers as a non-root user, `claude`
2.1.250, `codex-cli` 0.150.1, Docker 29.4.0. Tests drove `cat`, never a live agent
TUI. Anything not confirmed by a doc page or a local test is **UNVERIFIED**.

Goal: same run layout as the headless harness (`/work`, `/task:ro`, `/progress`,
`/agent-home`, `/out`), but the agent runs as an interactive TUI inside a named,
non-`--rm` container under a headless herdr server; the `/goal` prompt is
injected with `herdr agent prompt`; the launch script returns immediately; the
user later re-attaches with `docker exec -it … herdr` to watch, scroll, and
inspect while the agent is alive.

---

## 1. herdr — verified facts

### 1.1 Project

| Item | Value | Source |
|---|---|---|
| What | "Agent multiplexer that lives in your terminal": workspaces → tabs → panes, persistent background server, agent state detection (`idle`/`working`/`blocked`/`done`/`unknown`), CLI + NDJSON Unix-socket API | https://herdr.dev , `docs/concepts.mdx` |
| Version | 0.8.2 (released 2026-08-19; headless virtual terminal now 120×40 default) | https://github.com/herdrdev/herdr/releases/latest |
| License | Apache-2.0 (`LICENSE` in repo; site footer agrees). Some blog posts say AGPL — wrong | https://raw.githubusercontent.com/herdrdev/herdr/master/LICENSE |
| Install | `curl -fsSL https://herdr.dev/install.sh \| sh`; `brew install herdr`; `mise use -g herdr`; `nix run github:herdrdev/herdr/v0.x.y`; raw release assets `herdr-linux-x86_64`, `herdr-linux-aarch64`, `herdr-macos-*`, `herdr-windows-x86_64.zip` | https://herdr.dev/docs/install/ |
| Linux binary | **statically linked** ELF (`file`: "statically linked") → drops into any base image, no libc concerns | verified locally |
| crates.io `herdr` 0.1.0 | unrelated/stale; do not `cargo install` | `cargo search` |
| Config | `~/.config/herdr/config.toml` (`HERDR_CONFIG_PATH` overrides); defaults via `herdr --default-config` | `herdr --help` |
| Sessions | default session socket `~/.config/herdr/herdr.sock`; named session → `~/.config/herdr/sessions/<name>/{herdr.sock,herdr-client.sock,herdr-server.log,session.json}`. Resolution: `--session <name>` > `HERDR_SOCKET_PATH` > `HERDR_SESSION` > default | `docs/socket-api.mdx`; verified locally + in Docker |
| Headless server | `herdr server` "runs the headless server explicitly. Use it for supervised or service-style setups." It does **not** daemonize (`status.server.capabilities.detached_server_daemon: false`) → background it yourself | `docs/cli-reference.mdx`; verified |
| Docker / TTY / root | `herdr server` runs as a non-root user inside `docker run -d` **without `-t`**; `docker exec -u agent -e HERDR_SESSION=<name> … herdr …` controls it; root can control it via `HERDR_SOCKET_PATH=/home/agent/.config/herdr/sessions/<name>/herdr.sock` | verified locally (Debian container) |
| `docker stop`/`start` | server and all pane processes die; on `start` the entrypoint starts a fresh server. `session.json` (layout snapshot) is written on graceful `herdr server stop`, **not** observed after a plain `docker stop` → trap SIGTERM and call `herdr server stop` | verified locally |
| Restart semantics | "Snapshot restore does not preserve running shells… Panes … come back as new shells in their saved directories." Agent conversations resume only via native agent session restore (integration-reported session id): Claude Code → `claude --resume <id>` (integration v6+), Codex → `codex resume <id>` (v5+); `[session] resume_agents_on_restore = true` by default | `docs/session-state.mdx` |
| Scrollback | `advanced.scrollback_limit_bytes` default `10000000` per pane; `[experimental] pane_history = true` persists recent screen to `session-history.json` across server restarts | `docs/config-reference.json`, `docs/session-state.mdx` |
| Headless size | `[server] headless_cols = 120`, `headless_rows = 40` ("Virtual terminal … when no client is attached") | config reference |
| Background network | `[update] version_check = true`, `manifest_check = true` (herdr.dev polls) → set both `false` under the API-only firewall | `herdr --default-config` |

### 1.2 CLI surface used by the harness (exact, from `herdr <group>` and `docs/cli-reference.mdx`)

```
herdr server                                   # headless server (foreground)
herdr server stop
herdr status [--json]
herdr session list [--json] | attach <name> | stop <name> [--json] | delete <name> [--json]
herdr workspace create [--cwd PATH] [--label TEXT] [--env KEY=VALUE] [--focus|--no-focus]   # → .result.root_pane.pane_id
herdr pane list | get <pane_id> | process-info [--pane ID] | layout
herdr pane run <pane_id> <command>             # text + Enter atomically, honors bracketed paste
herdr pane send-text <pane_id> <text>          # literal, no Enter
herdr pane send-keys <pane_id> <key> [key ...] # enter, esc, ctrl+c, shift+tab, ...
herdr pane read <pane_id> [--source visible|recent|recent-unwrapped|detection] [--lines N] [--format text|ansi] [--raw]
herdr pane wait-output <pane_id> (--match TEXT | --regex PATTERN) [--source ...] [--lines N] [--timeout MS]
herdr agent list | get <target> | explain <target> [--json] | rename <target> <name>|--clear
herdr agent start <name> --kind claude|codex|... --pane ID [--timeout MS] [-- <agent-args...>]
herdr agent prompt <target> <text> [--wait] [--until STATUS]... [--timeout MS]
herdr agent wait <target> [--until STATUS]... [--timeout MS]
herdr agent send-keys <target> <key> [key ...]
herdr agent read <target> [--source ...] [--lines N]
herdr agent attach <target> [--takeover]       # direct terminal attach; detach ctrl+b q
herdr integration install claude|codex | status
herdr api snapshot | schema [--json]
```

Semantics that matter (quotes from `docs/agent-automation.mdx` / `cli-reference.mdx`):

- `agent start`: "returns only after Herdr detects the expected agent in the same terminal and marks it ready for interactive input… If detection reports `blocked` during startup, the command returns `agent_not_ready`"; default timeout 30 s, `--timeout` 3001–300000 ms; "Arguments after `--` are passed unchanged to that executable."
- `agent prompt`: "honors live bracketed-paste mode and sends text followed by encoded Enter after a short delay, including while the agent is working. If the agent is already `blocked`, it returns `agent_blocked` without sending input." With `--wait`, a prompt from a non-working state "must produce a lifecycle change within five seconds. Otherwise… `agent_prompt_stalled`"; then waits for settled `idle`/`done`/`blocked` (defaults). "It does not track individual turns."
- States: "`idle` means the agent is ready for input and its tab has been seen in the focused Herdr UI. `done` is the same underlying idle state after background work finishes, until that tab is focused… Reading through the CLI does not mark it seen. `blocked` means Herdr recognized an approval or question UI. `unknown` … does not prove successful completion." Blocked detection is "deliberately strict"; unknown dialogs may show as `idle`.
- Claude Code / Codex detection = **screen manifest** (bottom-buffer snapshot + foreground process name); the `claude`/`codex` integrations only report **session identity** (for restore), not lifecycle (`docs/agents.mdx`, `docs/integrations.mdx`).
- Wrapper hint: "Set `HERDR_AGENT=<agent>` on the wrapper command to tell Herdr which existing agent screen manifest to use" (`docs/agents.mdx`). Verified: `pane run … "HERDR_AGENT=claude script -qfec 'cat -v' /out/tui.log"` → `agent list` shows `"agent":"claude"` for that pane.
- Reads: default 80 rows; "`--lines N` selects the last N rendered terminal rows"; `recent-unwrapped` "Best for logs". Alternate-screen agents (fullscreen Claude Code) are read by herdr driving the agent's mouse-scroll **only while idle** — avoid by running Claude Code's classic renderer (§2).
- Waits "wait indefinitely when `--timeout` is omitted"; server errors = JSON on stderr, exit 1; usage errors exit 2.
- Events: socket `events.subscribe` with `pane.agent_status_changed`, `pane.output_matched`, `pane.exited` (`docs/socket-api.mdx`) — CLI has no subscribe command; `agent wait` covers the harness need.
- Observed quirk (Docker, no client ever attached): `pane read --source recent|recent-unwrapped` printed nothing while `visible`, `detection`, and `pane wait-output` (which uses recent-unwrapped internally) returned the text. On macOS the same reads worked. **UNVERIFIED** cause; `status.sh` uses `visible`/`detection` plus the `script` log, not `recent`.
- No built-in "log pane to file" command → use `script(1)` inside the pane (verified: `/usr/bin/script` exists in `debian:bookworm-slim`; log contains the raw stream incl. CRs).

### 1.3 Claude Code interactive facts (kept from the earlier pass; all official unless marked)

| Fact | Source |
|---|---|
| Enter submits; newline = Ctrl+J / `\`+Enter / paste; pastes >800 chars or >2 lines collapse to `[Pasted text #N]` but are sent whole | https://code.claude.com/docs/en/interactive-mode , /docs/en/terminal-config#paste-large-content |
| `claude "<prompt>"` accepts an initial positional prompt in interactive mode (`claude --help`); whether `"/goal …"` as that arg sets the goal is **UNVERIFIED** | local |
| `--max-turns` / `--max-budget-usd` are print-mode only → no hard cap in headed mode | https://code.claude.com/docs/en/cli-reference |
| `--session-id <uuid>` works interactively → transcript path known up front: `$CLAUDE_CONFIG_DIR/projects/<dir>/<id>.jsonl`; `CLAUDE_CODE_PROJECT_DIR_NAME=work` (with `CLAUDE_CONFIG_DIR`) pins `<dir>` to `work` (≥2.1.234) | https://code.claude.com/docs/en/sessions |
| **Fullscreen renderer is the default** on first launch ≥2.1.239 when feature flags aren't fetched (telemetry off) → conversation on the alternate screen. Force classic: `CLAUDE_CODE_DISABLE_ALTERNATE_SCREEN=1` and `"tui": "default"` (also suppresses the startup "switch to fullscreen?" dialog) | https://code.claude.com/docs/en/fullscreen#fullscreen-by-default , settings-reference `tui` |
| Bypass-permissions confirmation dialog on every interactive launch unless `skipDangerousModePermissionPrompt: true` in **user/local/managed** settings ("An untrusted repository can't skip the dialog for you") | https://code.claude.com/docs/en/settings-reference#skipdangerousmodepermissionprompt , issues #25503 #52506 |
| Workspace trust dialog: interactive only, keyed on git root; pre-accept with `projects["<repo-root>"].hasTrustDialogAccepted: true` in `~/.claude.json` | https://code.claude.com/docs/en/permissions#what-runs-before-you-trust-a-folder |
| `~/.claude.json` lives at `$CLAUDE_CONFIG_DIR/.claude.json` when the var is set | https://code.claude.com/docs/en/devcontainer#persist-authentication-and-settings-across-rebuilds |
| `hasCompletedOnboarding: true` (top-level) skips onboarding — key exists in a real `.claude.json`; semantics community-only (jedi.be, etokarev/claude-code-docker) | https://jedi.be/blog/2025/automating-claude-code-configuration/ |
| `customApiKeyResponses.approved: ["<last 20 chars>"]` pre-answers "Do you want to use this API key?" — **UNVERIFIED** (tessl.io) | https://tessl.io/blog/configuring-claude-code |
| `theme` is a settings.json key | settings-reference |
| `/goal` interactive = same prompt-based Stop hook as `-p`; extras: verdicts shown in transcript (Ctrl+O), `◎ /goal active` indicator, idle check-ins (`CLAUDE_CODE_GOAL_CHECKIN_MINUTES=0` disables), and after several no-tool turns it "returns control to you with the goal still set" (TUI idles) | https://code.claude.com/docs/en/goal |
| Goal restored on `--resume`/`--continue` (counters reset); permission mode **not** restored from bypass → pass `--dangerously-skip-permissions` again | goal page, sessions#permission-mode-on-resume |
| Transcript goal events (observed 2.1.2xx, format unstable by policy): `{"type":"attachment","attachment":{"type":"goal_status","met":false,"sentinel":true,"condition":…}}` on set; per verdict `…"goal_status","met":true|false,"reason":…`; plus `{"type":"system","subtype":"stop_hook_summary"}` | local transcript |
| `--dangerously-skip-permissions` rejected as root | https://code.claude.com/docs/en/devcontainer#run-without-permission-prompts |

### 1.4 Codex TUI facts

`codex "<prompt>"` starts the TUI with an initial prompt; `codex resume <id|name>`/`--last`; `--no-alt-screen` "preserving terminal scrollback history"; `--dangerously-bypass-approvals-and-sandbox`; `-a never`; `-C /work`; `--add-dir` (local `codex --help`, https://learn.chatgpt.com/docs/developer-commands?surface=cli). Trust prompt on first launch is **not** skipped by the bypass flag; pre-seed `[projects."/work"] trust_level = "trusted"`; `notice.hide_full_access_warning = true`; `tui.show_tooltips`, `tui.animations`, `tui.alternate_screen` exist (https://learn.chatgpt.com/docs/config-file/config-reference.md , https://github.com/openai/codex/issues/14547). `/goal` exists in the Codex TUI (0.128+). Sending `/goal` via `agent prompt`, and goal events in rollout JSONL: **UNVERIFIED**.

---

## 2. Requirement → herdr mapping

| Brief requirement | herdr mechanism | Status |
|---|---|---|
| Agent TUI keeps running after launch script returns | `herdr server` (backgrounded, PID-1-supervised) hosts the pane; `docker run -d` no `-t`; script exits | verified with `cat` |
| Non-`--rm` container, re-attachable, agent alive | `docker exec -it -u agent -e HERDR_SESSION=agent <c> herdr` (full TUI, sidebar, scroll) or `herdr agent attach task`; detach `ctrl+b q` | docs; interactive attach through `docker exec -it` **UNVERIFIED by test** (no TTY here); users run herdr inside Docker per herdr issue #2603 |
| Programmatic `/goal` injection | `herdr agent prompt task "$PROMPT"` (bracketed-paste aware, Enter after delay, refuses when `blocked`) | docs; Enter-vs-slash-popup for `/goal …` **UNVERIFIED** |
| Full scrollback / continuous log | `script -qfec '<agent cmd>' /out/tui.log` inside the pane (raw stream, from first byte) + herdr scrollback (`scrollback_limit_bytes` raised) + `pane read --source visible|detection` snapshots | verified (`script`), reads verified |
| Session `.jsonl` transcript | unchanged: `/agent-home/projects/work/<session-id>.jsonl` | official |
| Completion detection | transcript `goal_status` verdict (authoritative) → `herdr agent wait task --until done --until idle --until blocked` (settled state) → `pane wait-output --regex '^GOAL_RESULT'` → `agent get` status | docs + local transcript format |
| Skip onboarding/trust/bypass dialogs | pre-seeded `settings.json` + `.claude.json` (§3); any remaining dialog shows as `blocked` (or `idle`) → `agent prompt` returns `agent_blocked` and `run-headed.sh` prints the screen | official + community |
| Inspect container state with agent alive | second `docker exec -it -u agent <c> bash -l`, or a second herdr pane (`herdr pane split w1:p1 --direction down --cwd /work --no-focus`) | docs |
| Lifecycle | `docker stop` → trap → `herdr server stop` (writes `session.json`); `docker start` → fresh server; agent conversation continues only via explicit `claude --resume <id> --dangerously-skip-permissions …` (§8) | verified / docs |

---

## 3. Config pre-seeding recipe

### 3.1 Claude Code (`agent-home/`, mounted at `/agent-home`, `CLAUDE_CONFIG_DIR=/agent-home`)

`agent-home/settings.json` (user scope — the only scope that may skip the bypass dialog):

```json
{
  "skipDangerousModePermissionPrompt": true,
  "theme": "dark",
  "tui": "default",
  "cleanupPeriodDays": 3650,
  "env": { "CLAUDE_CODE_GOAL_CHECKIN_MINUTES": "0" }
}
```

`agent-home/.claude.json` († = community-documented key only):

```json
{
  "hasCompletedOnboarding": true,
  "lastOnboardingVersion": "2.1.250",
  "numStartups": 1,
  "projects": {
    "/work": {
      "hasTrustDialogAccepted": true,
      "hasCompletedProjectOnboarding": true,
      "allowedTools": []
    }
  },
  "customApiKeyResponses": { "approved": ["<last20(ANTHROPIC_API_KEY)>"], "rejected": [] }
}
```

`hasTrustDialogAccepted` official (keyed on git root = `/work`, which the harness `git init`s); `hasCompletedOnboarding` †, `hasCompletedProjectOnboarding` †, `customApiKeyResponses` †.

Optional: `herdr integration install claude` in the entrypoint (needs `CLAUDE_CONFIG_DIR` to exist; "writes `hooks/herdr-agent-state.sh` and updates `settings.json` with Herdr hook entries" — a SessionStart-style hook reporting the session id to the herdr socket). It only matters for herdr's automatic `claude --resume` after a server restart, which the harness does explicitly anyway (§8) — and it adds a hook to the experiment input. Default: **off**.

Environment (Dockerfile): `CLAUDE_CONFIG_DIR=/agent-home`, `CLAUDE_CODE_PROJECT_DIR_NAME=work`, `CLAUDE_CODE_DISABLE_ALTERNATE_SCREEN=1`, `CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1`, `DISABLE_AUTOUPDATER=1`, `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, `DISABLE_TELEMETRY=1`, `DISABLE_ERROR_REPORTING=1`, `TERM=xterm-256color`, `LANG=C.UTF-8`, `COLORTERM=truecolor`. No `--bare` (kills hooks → `/goal`).

Why classic renderer: herdr host scrollback + `pane read` only see main-screen output; alternate-screen history is readable only through herdr's mouse-scroll trick while idle. **UNVERIFIED**: whether herdr's Claude screen manifest classifies `idle`/`working`/`blocked` equally well on the classic renderer — check with `herdr agent explain task --json` on the first run; the `script` log and the transcript don't depend on it.

### 3.2 herdr (`/home/agent/.config/herdr/config.toml`, baked into the image)

```toml
[update]
version_check = false
manifest_check = false
[server]
headless_cols = 200
headless_rows = 60
[session]
resume_agents_on_restore = false   # harness does explicit resume (§8)
[experimental]
pane_history = true                # replay recent screen after a server restart
[advanced]
scrollback_limit_bytes = 200000000
```

### 3.3 Codex (`$CODEX_HOME/config.toml`, copied at entry)

```toml
approval_policy = "never"
sandbox_mode    = "danger-full-access"
[features]
goals = true
[projects."/work"]
trust_level = "trusted"
[notice]
hide_full_access_warning = true
[tui]
show_tooltips = false
animations = false
```

Auth: `printenv OPENAI_API_KEY | codex login --with-api-key` in the entrypoint (writes `$CODEX_HOME/auth.json`).

---

## 4. Dockerfile deltas vs the headless sketch

```dockerfile
# experiments/images/claude/Dockerfile — delta only; base as in container-harness.md §6
ARG HERDR_VERSION=0.8.2
ARG TARGETARCH                       # amd64|arm64 (buildx sets it)
RUN apt-get update && apt-get install -y --no-install-recommends bsdutils jq locales \
    && rm -rf /var/lib/apt/lists/*   # bsdutils = script(1); already in debian:bookworm-slim
RUN case "$TARGETARCH" in amd64) A=x86_64;; arm64) A=aarch64;; esac \
    && curl -fsSL -o /usr/local/bin/herdr \
       https://github.com/herdrdev/herdr/releases/download/v${HERDR_VERSION}/herdr-linux-${A} \
    && chmod 755 /usr/local/bin/herdr && herdr --version
ENV TERM=xterm-256color LANG=C.UTF-8 LC_ALL=C.UTF-8 COLORTERM=truecolor \
    CLAUDE_CODE_PROJECT_DIR_NAME=work \
    CLAUDE_CODE_DISABLE_ALTERNATE_SCREEN=1 \
    CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1 \
    HERDR_SESSION=agent
COPY --chown=agent common/herdr-config.toml /home/agent/.config/herdr/config.toml
COPY common/entrypoint-headed.sh /usr/local/bin/entrypoint-headed.sh
RUN chmod +x /usr/local/bin/entrypoint-headed.sh
ENTRYPOINT ["/usr/local/bin/entrypoint-headed.sh"]
```

herdr's sockets live in `/home/agent/.config/herdr/sessions/agent/` — container-local (not on a macOS bind mount, short path). `HERDR_SESSION=agent` is in the image env, so every `docker exec -u agent` resolves the right socket; root execs pass `HERDR_SOCKET_PATH=/home/agent/.config/herdr/sessions/agent/herdr.sock`.

---

## 5. Entrypoint (headed)

```bash
#!/usr/bin/env bash
# /usr/local/bin/entrypoint-headed.sh — PID 1. Root: firewall; then herdr server + agent pane as `agent`.
set -euo pipefail
export HERDR_SESSION=agent
if [ "${NET_MODE:-api}" = "api" ]; then
  ALLOWED_DOMAINS="${ALLOWED_DOMAINS:-api.anthropic.com}" /usr/local/bin/init-firewall.sh
fi
if [ -n "${CODEX_HOME:-}" ]; then
  [ -f /etc/codex-config.toml ] && [ ! -f "$CODEX_HOME/config.toml" ] && cp /etc/codex-config.toml "$CODEX_HOME/config.toml"
  chown -R agent "$CODEX_HOME"
  [ -n "${OPENAI_API_KEY:-}" ] && gosu agent bash -c 'printenv OPENAI_API_KEY | codex login --with-api-key' || true
fi
: "${AGENT_CMD:?}"        # full agent command line, built by run-headed.sh
: "${AGENT_KIND:?}"       # claude | codex  (HERDR_AGENT hint for the script(1) wrapper)

H() { gosu agent env HOME=/home/agent HERDR_SESSION=agent herdr "$@"; }
# 1. headless server (does not daemonize) — keep its pid, log to /out
gosu agent env HOME=/home/agent HERDR_SESSION=agent herdr server >/out/herdr-server.log 2>&1 &
SRV=$!
for i in $(seq 1 50); do H status --json >/dev/null 2>&1 && break; sleep 0.2; done
# 2. one workspace rooted at /work → root pane id
PANE=$(H workspace create --cwd /work --label task --no-focus | jq -r '.result.root_pane.pane_id')
printf '%s\n' "$PANE" > /out/pane-id
# 3. launch the agent under script(1) so /out/tui.log holds the raw stream from byte 0;
#    HERDR_AGENT tells herdr which screen manifest to use behind the wrapper (docs/agents.mdx)
H pane run "$PANE" "HERDR_AGENT=$AGENT_KIND exec script -qfec $(printf %q "$AGENT_CMD") /out/tui.log"
# 4. supervise: docker stop → SIGTERM → graceful server stop (writes session.json), then exit
trap 'H server stop >/dev/null 2>&1 || kill $SRV 2>/dev/null; exit 0' TERM INT
wait $SRV
```

Alternative to step 3 (no raw log, but herdr-native readiness): `H agent start task --kind "$AGENT_KIND" --pane "$PANE" --timeout 120000 -- <args>` — returns when the agent is detected and idle. The harness prefers the `script` wrapper because the brief asks for a continuous log; readiness is then done by `run-headed.sh` with `agent wait`.

When the agent process exits, the pane returns to the shell prompt (herdr keeps the pane and its scrollback; the server keeps running) — the container stays up until `docker stop`.

---

## 6. `run-headed.sh` sketch

```bash
#!/usr/bin/env bash
# usage: run-headed.sh <claude|codex> <TASK-ID> [--model M] [--net api|all]
set -euo pipefail
AGENT=$1; TASK=$2; shift 2
MODEL=""; NET_MODE=api
while [ $# -gt 0 ]; do case $1 in
  --model) MODEL=$2; shift 2;; --net) NET_MODE=$2; shift 2;; *) echo "bad arg $1"; exit 2;; esac; done

ROOT=$(cd "$(dirname "$0")" && pwd)
TASK_DIR=$ROOT/tasks/$TASK
FIXTURE=$ROOT/fixtures/$(cat "$TASK_DIR/fixture")
RUN_ID=$(date -u +%Y%m%d-%H%M%S)-$AGENT-$TASK
RUN=$ROOT/runs/$RUN_ID
CNAME=harness-$RUN_ID
mkdir -p "$RUN"/{workspace,agent-home,out,task-snapshot,progress}

# 1. inputs (identical to the headless run.sh)
cp -a "$TASK_DIR"/. "$RUN/task-snapshot/"
cp -a "$FIXTURE"/. "$RUN/workspace/"
"$ROOT/../reference/task-template/task-lint.sh" "$TASK_DIR" >"$RUN/lint.log"      # refuse to dispatch an invalid contract
"$ROOT/../reference/task-template/progress-init.sh" "$TASK_DIR" -o "$RUN/progress/progress.md"
( cd "$RUN/workspace" && git init -q && git add -A && git commit -qm baseline --no-verify && git tag baseline )
"$ROOT/lib/hash-protected.sh" "$RUN" before > "$RUN/hashes-before.txt"

# 2. prompt = the /goal block of reference/goal-prompt.md collapsed to one line
#    (agent prompt handles multi-line via bracketed paste, but one line avoids the [Pasted text] chip)
PROMPT=$(awk '/^```text/{f=1;next} /^```/{f=0} f' "$ROOT/../reference/goal-prompt.md" | tr '\n' ' ' | sed 's/  */ /g; s/ $//')
printf '%s\n' "$PROMPT" > "$RUN/prompt.txt"
SESSION_ID=$(uuidgen | tr A-Z a-z)

# 3. pre-seed agent home (§3)
case $AGENT in
  claude)
    KEY_TAIL=${ANTHROPIC_API_KEY: -20}
    cat > "$RUN/agent-home/settings.json" <<EOF
{"skipDangerousModePermissionPrompt":true,"theme":"dark","tui":"default",
 "cleanupPeriodDays":3650,"env":{"CLAUDE_CODE_GOAL_CHECKIN_MINUTES":"0"}}
EOF
    cat > "$RUN/agent-home/.claude.json" <<EOF
{"hasCompletedOnboarding":true,"lastOnboardingVersion":"2.1.250","numStartups":1,
 "projects":{"/work":{"hasTrustDialogAccepted":true,"hasCompletedProjectOnboarding":true,"allowedTools":[]}},
 "customApiKeyResponses":{"approved":["$KEY_TAIL"],"rejected":[]}}
EOF
    AGENT_CMD="claude --dangerously-skip-permissions --session-id $SESSION_ID --add-dir /task --add-dir /progress ${MODEL:+--model $MODEL}"
    ENVS=(-e ANTHROPIC_API_KEY -e ALLOWED_DOMAINS=api.anthropic.com -e CLAUDE_CONFIG_DIR=/agent-home)
    IMAGE=harness-claude:latest;;
  codex)
    AGENT_CMD="codex --dangerously-bypass-approvals-and-sandbox --no-alt-screen -C /work --add-dir /task --add-dir /progress ${MODEL:+-m $MODEL}"
    ENVS=(-e OPENAI_API_KEY -e ALLOWED_DOMAINS=api.openai.com -e CODEX_HOME=/agent-home)
    IMAGE=harness-codex:latest;;
esac

# 4. container: named, NOT --rm, detached, no -t (herdr server needs no TTY)
CAPS=(); [ "$NET_MODE" = api ] && CAPS=(--cap-add NET_ADMIN --cap-add NET_RAW)
docker run -d --name "$CNAME" "${CAPS[@]}" "${ENVS[@]}" \
  -e NET_MODE="$NET_MODE" -e AGENT_CMD="$AGENT_CMD" -e AGENT_KIND="$AGENT" \
  -v "$RUN/workspace:/work" -v "$TASK_DIR:/task:ro" -v "$RUN/progress:/progress" \
  -v "$RUN/agent-home:/agent-home" -v "$RUN/out:/out" \
  --memory 4g --cpus 2 --pids-limit 2048 "$IMAGE" >/dev/null

H() { docker exec -u agent "$CNAME" herdr "$@"; }       # HERDR_SESSION=agent is image env
for i in $(seq 1 100); do [ -s "$RUN/out/pane-id" ] && break; sleep 0.5; done
PANE=$(cat "$RUN/out/pane-id")
jq -n --arg run "$RUN_ID" --arg container "$CNAME" --arg agent "$AGENT" --arg task "$TASK" \
      --arg model "$MODEL" --arg net "$NET_MODE" --arg session "$SESSION_ID" --arg pane "$PANE" \
      --arg start "$(date -u +%FT%TZ)" \
      '{run:$run,container:$container,agent:$agent,task:$task,model:$model,net:$net,session_id:$session,pane:$pane,start:$start}' \
      > "$RUN/meta.json"

# 5. readiness: herdr detects the agent (HERDR_AGENT hint) and classifies it idle
if ! H agent wait "$PANE" --until idle --timeout 180000 >/dev/null 2>"$RUN/out/wait-ready.err"; then
  echo "agent not idle after 180 s (dialog? auth?). Screen:"; H pane read "$PANE" --source visible; cat "$RUN/out/wait-ready.err"; exit 1
fi
H agent rename "$PANE" task >/dev/null                   # stable target name for the other scripts

# 6. inject the /goal prompt (text + Enter, bracketed-paste aware; refuses if a dialog is up)
if ! H agent prompt task "$PROMPT" >"$RUN/out/prompt.json" 2>"$RUN/out/prompt.err"; then
  echo "prompt refused:"; cat "$RUN/out/prompt.err"; H pane read "$PANE" --source visible; exit 1
fi
# confirm the goal was accepted: transcript gets the goal_status sentinel line within seconds
for i in $(seq 1 15); do
  sleep 2
  [ "$AGENT" = claude ] && grep -q '"goal_status"' "$RUN/agent-home/projects/work/$SESSION_ID.jsonl" 2>/dev/null && { echo "goal accepted"; break; }
  [ "$AGENT" = codex ] && [ "$(H agent get task | jq -r .result.agent.agent_status)" = working ] && { echo "prompt consumed"; break; }
  # Enter eaten by the slash-command popup? (UNVERIFIED failure mode) — resend once
  [ $i = 5 ] && H pane read "$PANE" --source visible | grep -qF "${PROMPT:0:40}" && H agent send-keys task enter
done

cat <<EOF
run:        $RUN
container:  $CNAME
attach:     $ROOT/attach.sh $RUN_ID       # docker exec -it -u agent $CNAME herdr   (detach: ctrl+b q)
status:     $ROOT/status.sh $RUN_ID [--wait]
raw log:    $RUN/out/tui.log              # script(1) stream, from the first byte
transcript: $RUN/agent-home/projects/work/$SESSION_ID.jsonl
EOF
```

Notes:

- `agent prompt` without `--wait`: `--wait` would block until the first settled state, which under `/goal` may be the end of the whole loop (or an idle blip between turns) — the launcher must return, so polling is left to `status.sh`.
- No `pane send-text` + `send-keys enter` for the prompt: the CLI reference says "Prefer [`pane run`/`agent prompt`] over `send-text` plus `send-keys Enter`… the separate send operations remain low-level and non-submitting." `agent prompt` also adds the delay before Enter that tmux-style drivers had to hand-roll.
- Turn cap: none in headed mode (`--max-turns` is print-only). "Stop after 40 turns" in the condition is model-honoured; `status.sh --kill-after N` is the hard stop.

---

## 7. `attach.sh`

```bash
#!/usr/bin/env bash
# usage: attach.sh <RUN_ID> [--direct]      detach with ctrl+b q — never ctrl+c (that interrupts the agent)
set -euo pipefail
RUN=$(cd "$(dirname "$0")" && pwd)/runs/$1
CNAME=$(jq -r .container "$RUN/meta.json")
docker start "$CNAME" >/dev/null 2>&1 || true   # no-op if running; see §8 for what a restart means
if [ "${2:-}" = --direct ]; then
  exec docker exec -it -u agent -e TERM="${TERM:-xterm-256color}" "$CNAME" herdr agent attach task
fi
exec docker exec -it -u agent -e TERM="${TERM:-xterm-256color}" "$CNAME" herdr      # full TUI on session `agent`
```

Inside: mouse wheel / `prefix+e` (`edit_scrollback`) to scroll; the sidebar shows the agent's state. `herdr agent attach task` is the single-pane view ("Detach from direct attach with `ctrl+b q`"). A shell next to the agent without disturbing it: `docker exec -it -u agent <c> bash -l` (then `git -C /work status`, `cat /progress/progress.md`), or `herdr pane split w1:p1 --direction down --cwd /work --no-focus` from inside the TUI.

---

## 8. `status.sh` (completion detection from outside)

```bash
#!/usr/bin/env bash
# usage: status.sh <RUN_ID> [--wait] [--kill-after MIN]
set -euo pipefail
ROOT=$(cd "$(dirname "$0")" && pwd); RUN=$ROOT/runs/$1; shift
WAIT=0; KILL_AFTER=0
while [ $# -gt 0 ]; do case $1 in --wait) WAIT=1; shift;; --kill-after) KILL_AFTER=$2; shift 2;; *) exit 2;; esac; done
CNAME=$(jq -r .container "$RUN/meta.json"); AGENT=$(jq -r .agent "$RUN/meta.json")
SID=$(jq -r .session_id "$RUN/meta.json"); PANE=$(jq -r .pane "$RUN/meta.json")
TR=$RUN/agent-home/projects/work/$SID.jsonl
H() { docker exec -u agent "$CNAME" herdr "$@" 2>/dev/null; }

check() {
  local state=RUNNING reason="" hstate="" result=""
  [ "$(docker inspect -f '{{.State.Running}}' "$CNAME" 2>/dev/null)" = true ] || { echo '{"state":"CONTAINER_STOPPED"}'; return; }
  hstate=$(H agent get task | jq -r '.result.agent.agent_status // "none"')       # idle|working|blocked|done|unknown|none
  [ "$hstate" = none ] && state=AGENT_EXITED                                       # pane back at the shell, no agent
  # 1. authoritative (Claude): last evaluator verdict in the transcript
  if [ "$AGENT" = claude ] && [ -f "$TR" ]; then
    local g; g=$(jq -rc 'select(.type=="attachment" and .attachment.type=="goal_status" and (.attachment.sentinel|not))
                          | {met:.attachment.met, reason:(.attachment.reason//"")}' "$TR" | tail -n1)
    if [ -n "$g" ]; then reason=$(jq -r .reason <<<"$g"); [ "$(jq -r .met <<<"$g")" = true ] && state=GOAL_MET; fi
    grep -q 'Goal cleared after an unrecoverable error' "$RUN/out/tui.log" 2>/dev/null && state=GOAL_CLEARED_ERROR
  fi
  # 2. agent-side signal (both agents): GOAL_RESULT line in the raw log (strip CR)
  result=$(tr -d '\r' < "$RUN/out/tui.log" 2>/dev/null | grep -E '^GOAL_RESULT' | tail -n1 || true)
  # 3. herdr says settled (idle/done/blocked) and nothing else fired → idle with goal left set, or a dialog
  [ "$state" = RUNNING ] && case $hstate in idle|done) state=IDLE;; blocked) state=BLOCKED;; esac
  jq -n --arg state "$state" --arg herdr "$hstate" --arg reason "$reason" --arg result "$result" \
        --argjson verdicts "$( [ -f "$TR" ] && jq -c 'select(.attachment.type=="goal_status" and (.attachment.sentinel|not))|.attachment.met' "$TR" | jq -s length || echo 0)" \
        '{state:$state,herdr_status:$herdr,goal_reason:$reason,goal_result_line:$result,goal_verdicts:$verdicts}'
}

if [ $WAIT = 1 ]; then
  START=$(date +%s)
  while :; do
    # block on herdr's server-side wait (event-driven) instead of tight polling; 5-min slices so --kill-after can fire
    H agent wait task --until idle --until done --until blocked --timeout 300000 >/dev/null || true
    S=$(check); echo "$S"
    [ "$(jq -r .state <<<"$S")" = RUNNING ] || break
    if [ "$KILL_AFTER" -gt 0 ] && [ $(( ($(date +%s)-START)/60 )) -ge "$KILL_AFTER" ]; then
      H agent prompt task '/goal clear' >/dev/null || true; echo '{"state":"KILLED_TIMEOUT"}'; break
    fi
  done
else check; fi
H pane read "$PANE" --source visible > "$RUN/out/screen.txt" 2>/dev/null || true   # last rendered screen for offline reading
```

Signals, in order of trust:

1. Transcript `goal_status` with `met:true` (Claude) — the evaluator's verdict, `reason` explains it (`sentinel:true` lines are the "goal set" marker).
2. `GOAL_RESULT` line in `/out/tui.log` (both agents; agent-authored → "agent thinks it finished", not `done`).
3. herdr `agent_status`: `idle`/`done` = settled (goal met, goal left set after no-tool turns, or agent stopped); `blocked` = a dialog herdr recognises; `unknown` = agent present but unclassified (not completion); target gone = agent exited.
4. `IDLE`/`BLOCKED` with no verdict → attach and look.

The trusted gate is unchanged: on a terminal state run the headless note's step 6 (`git diff baseline`, hashes, `verify.sh` in a `--network none` container on the host copy). The container stays up meanwhile.

Lifecycle:

- `docker stop` → entrypoint trap → `herdr server stop` (graceful; writes `session.json`) → container exits. Bind mounts keep workspace, `/progress`, `/agent-home` (transcript), `/out/tui.log`. Pane processes are gone.
- `docker start` → entrypoint re-runs: firewall, fresh `herdr server` (restores layout from `session.json`, replays recent screen if `pane_history`), and launches `AGENT_CMD` again — a **new** conversation unless `AGENT_CMD` is the resume form. Make the entrypoint pick it: if `$CLAUDE_CONFIG_DIR/projects/work/<id>.jsonl` exists, run `claude --resume <id> --dangerously-skip-permissions --add-dir /task --add-dir /progress` (goal is restored, permission mode is not, `--add-dir` must be repeated — all official). Codex: `codex resume <id>`. herdr's own `resume_agents_on_restore` is disabled (§3.2) because it would run bare `claude --resume <id>` without the flags.
- Never `docker attach` (PID 1 is the supervisor). Never Ctrl-C inside the attached herdr (interrupts the agent); detach with `ctrl+b q`.
- Cleanup: `docker rm -f harness-<run-id>` once the run dir has everything (it does; only herdr's sockets/`session.json` are container-local).

---

## 9. What you can inspect after (and during) the run

| What | Where / how |
|---|---|
| Live TUI, agent state in the sidebar, scrollback | `attach.sh <run>`; `attach.sh <run> --direct` for the bare terminal |
| Raw terminal stream from the first byte (colours, timing, dialogs) | `runs/<id>/out/tui.log` (`script` output; `tr -d '\r'` / `sed 's/\x1b\[[0-9;?]*[A-Za-z]//g'` to read) |
| Rendered screen snapshot | `runs/<id>/out/screen.txt` (from `status.sh`) or `docker exec -u agent <c> herdr pane read w1:p1 --source visible` |
| herdr's view of the agent | `herdr agent get task`, `herdr agent explain task --json` (why it classified idle/working/blocked) |
| Session transcript (tool calls, results, goal verdicts, compactions) | `runs/<id>/agent-home/projects/work/<session-id>.jsonl` (Claude) · `runs/<id>/agent-home/sessions/**/rollout-*.jsonl` (Codex) |
| Goal verdict history | `jq 'select(.attachment.type=="goal_status")' <transcript>` |
| Agent state file | `runs/<id>/progress/progress.md` |
| Workspace diff / untracked files | `git -C runs/<id>/workspace diff baseline`, `git status --porcelain`; or `docker exec -it -u agent <c> bash -l` |
| Protected-file integrity | `lib/hash-protected.sh <run> after` vs `hashes-before.txt`; `/task` is `:ro` |
| Verifier output the agent saw | grep `verify.sh` / `DONE` in `tui.log` or the transcript |
| Effective config (did the seed work?) | diff `runs/<id>/agent-home/.claude.json` / `settings.json` against the seed |
| herdr server log / layout snapshot | `runs/<id>/out/herdr-server.log`; in-container `/home/agent/.config/herdr/sessions/agent/session.json` |
| Ask the finished session questions | `docker exec -u agent <c> claude -p --resume <session-id> --output-format json "summarize what you changed"` (after the run — both write the same transcript) |

---

## 10. Codex TUI equivalents (brief)

Same container, server, pane, `script` wrapper (`HERDR_AGENT=codex`), attach, and lifecycle. `AGENT_CMD` = `codex --dangerously-bypass-approvals-and-sandbox --no-alt-screen -C /work --add-dir /task --add-dir /progress` (`--no-alt-screen` keeps the transcript in herdr scrollback). Pre-seed §3.3; auth via `codex login --with-api-key`. Prompt: either the Codex text from `reference/goal-prompt.md` as a plain prompt (one attempt) or `/goal <condition>` (TUI goals exist since 0.128) — an experiment variant, not a harness decision. Completion: no verified goal event in rollout JSONL → `GOAL_RESULT` in `tui.log` + herdr `idle`/`done`. Continue: `codex resume <id>`.

---

## 11. Open items for the first headed run

1. Which dialogs still appear with the §3 seed (API-key approval, onboarding, fullscreen offer, release notes) — attach right after `run-headed.sh`; fold missing keys into the seed. `agent prompt` returning `agent_blocked` is the tell.
2. `/goal …` via `agent prompt` sets the goal (transcript sentinel line appears) and the slash popup doesn't eat the Enter.
3. herdr's Claude/Codex screen manifest classifies correctly on the classic renderer / `--no-alt-screen` (`herdr agent explain task --json`); if `unknown` persists, `status.sh` still works from the transcript + `tui.log`.
4. `docker exec -it … herdr` attach quality (resize, mouse, detach) from macOS terminals.
5. `pane read --source recent*` empty in headless Linux (observed) — reproduce with a client attached; irrelevant while `visible`/`detection`/`wait-output` and the `script` log work.
6. `customApiKeyResponses` key format — if wrong, answer once by hand and copy the resulting `.claude.json` back into the seed.
