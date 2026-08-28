#!/usr/bin/env bash
# agent-launch.sh — agent supervisor, runs as user `agent` (exec'd from entrypoint.sh via gosu).
#
#   (no args; configured by env — both required)
#     AGENT_CMD    full agent command line, built by run-headed.sh
#     AGENT_KIND   claude | codex — HERDR_AGENT manifest hint for the script(1) wrapper
#
# Headless herdr server + one /work workspace; $AGENT_CMD runs in the root pane under script(1)
# so /out/tui.log holds the raw terminal stream from the first byte. docker stop → SIGTERM here →
# graceful `herdr server stop` (writes session.json).
set -Eeuo pipefail
export HOME=/home/agent
export HERDR_SESSION=agent

: "${AGENT_CMD:?AGENT_CMD not set}"
: "${AGENT_KIND:?AGENT_KIND not set}"

# 1. headless server (does not daemonize) — keep its pid, log to /out
herdr server >/out/herdr-server.log 2>&1 &
SRV=$!
for _ in $(seq 1 50); do herdr status >/dev/null 2>&1 && break; sleep 0.2; done
herdr status >/dev/null 2>&1 || {
  echo "agent-launch: herdr server did not come up — see /out/herdr-server.log" >&2
  kill "$SRV" 2>/dev/null || true
  exit 1
}

# 2. one workspace rooted at /work → root pane id
PANE=$(herdr workspace create --cwd /work --label task --no-focus | jq -r '.result.root_pane.pane_id')
if [ -z "$PANE" ] || [ "$PANE" = null ]; then
  echo "agent-launch: no pane id from workspace create" >&2
  herdr server stop >/dev/null 2>&1 || kill "$SRV" 2>/dev/null || true
  exit 1
fi
printf '%s\n' "$PANE" >/out/pane-id

# 3. agent under script(1): /out/tui.log = raw stream from byte 0; HERDR_AGENT tells herdr which
#    screen manifest to use behind the wrapper (docs/agents.mdx)
herdr pane run "$PANE" "HERDR_AGENT=$AGENT_KIND exec script -qfec $(printf %q "$AGENT_CMD") /out/tui.log"

# 4. supervise: SIGTERM (docker stop) → graceful server stop, then clean exit
trap 'herdr server stop >/dev/null 2>&1 || kill "$SRV" 2>/dev/null || true; exit 0' TERM INT
wait "$SRV"
