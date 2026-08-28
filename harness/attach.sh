#!/usr/bin/env bash
# attach.sh — re-attach to a headed run's live agent TUI (herdr) inside its container.
#
#   attach.sh <RUN_ID> [--direct]
#
# Detach with ctrl+b q — never ctrl+c (that interrupts the agent).
# --direct attaches to the single agent pane (herdr agent attach task); the default opens the
# full herdr TUI on session "agent" (sidebar with the agent state, scrollback).
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[[ $# -ge 1 && "$1" != --* ]] || { echo "usage: attach.sh <RUN_ID> [--direct]" >&2; exit 64; }
RUN="$ROOT/../experiments/runs/$1"
[[ -f "$RUN/meta.json" ]] || { echo "no such run: $RUN" >&2; exit 66; }
CNAME="$(jq -r .container "$RUN/meta.json")"
docker start "$CNAME" >/dev/null 2>&1 || true   # no-op if running; restart semantics: notes §8
if [[ "${2:-}" == "--direct" ]]; then
  exec docker exec -it -u agent -e TERM="${TERM:-xterm-256color}" "$CNAME" herdr agent attach task
fi
exec docker exec -it -u agent -e TERM="${TERM:-xterm-256color}" "$CNAME" herdr
