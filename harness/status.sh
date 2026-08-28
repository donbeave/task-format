#!/usr/bin/env bash
# status.sh — completion detection for one headed run, from outside the container.
#
#   status.sh <RUN_ID> [--wait] [--kill-after MIN]
#
# Signals, in order of trust (docs/research/notes/headed-herdr-harness.md §8):
#   1. transcript goal_status verdict with met:true (claude)          -> GOAL_MET
#   2. GOAL_RESULT line in the raw tui.log (both agents; agent-authored)
#   3. herdr agent_status: idle/done = settled, blocked = a dialog, target gone = agent exited
# The trusted gate stays the host-side verify.sh re-run (harness/README.md "Gate").
# Writes the last rendered screen to <run>/out/screen.txt on every invocation.
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[[ $# -ge 1 && "$1" != --* ]] || { echo "usage: status.sh <RUN_ID> [--wait] [--kill-after MIN]" >&2; exit 64; }
RUN="$ROOT/../experiments/runs/$1"; shift
WAIT=0; KILL_AFTER=0
while [[ $# -gt 0 ]]; do case $1 in
  --wait) WAIT=1; shift;;
  --kill-after) KILL_AFTER=$2; shift 2;;
  *) echo "bad arg $1" >&2; exit 64;;
esac; done
[[ -f "$RUN/meta.json" ]] || { echo "no such run: $RUN" >&2; exit 66; }
CNAME=$(jq -r .container "$RUN/meta.json")
AGENT=$(jq -r .agent "$RUN/meta.json")
SID=$(jq -r .session_id "$RUN/meta.json")
PANE=$(jq -r .pane "$RUN/meta.json")
TR="$RUN/agent-home/projects/work/$SID.jsonl"
H() { docker exec -u agent "$CNAME" herdr "$@" 2>/dev/null; }

check() {
  local state=RUNNING reason="" hstate="" result="" g="" verdicts=0
  if [[ "$(docker inspect -f '{{.State.Running}}' "$CNAME" 2>/dev/null)" != true ]]; then
    echo '{"state":"CONTAINER_STOPPED"}'; return
  fi
  hstate="$(H agent get task | jq -r '.result.agent.agent_status // "none"' || true)"
  [[ -n "$hstate" ]] || hstate=none                       # idle|working|blocked|done|unknown|none
  if [[ "$hstate" == none ]]; then state=AGENT_EXITED; fi # pane back at the shell, no agent
  # 1. authoritative (claude): last evaluator verdict in the transcript
  if [[ "$AGENT" == claude && -f "$TR" ]]; then
    g="$(jq -rc 'select(.type=="attachment" and .attachment.type=="goal_status" and (.attachment.sentinel|not))
                 | {met:.attachment.met, reason:(.attachment.reason//"")}' "$TR" 2>/dev/null | tail -n1 || true)"
    if [[ -n "$g" ]]; then
      reason=$(jq -r .reason <<<"$g")
      if [[ "$(jq -r .met <<<"$g")" == true ]]; then state=GOAL_MET; fi
    fi
    if grep -q 'Goal cleared after an unrecoverable error' "$RUN/out/tui.log" 2>/dev/null; then
      state=GOAL_CLEARED_ERROR
    fi
  fi
  # 2. agent-side signal (both agents): GOAL_RESULT line in the raw log (strip CR)
  result="$(tr -d '\r' < "$RUN/out/tui.log" 2>/dev/null | grep -E '^GOAL_RESULT' | tail -n1 || true)"
  # 3. herdr says settled and nothing else fired -> idle with goal left set, or a dialog
  if [[ "$state" == RUNNING ]]; then
    case $hstate in idle|done) state=IDLE;; blocked) state=BLOCKED;; esac
  fi
  if [[ "$AGENT" == claude && -f "$TR" ]]; then
    verdicts="$( { jq -c 'select(.attachment.type=="goal_status" and (.attachment.sentinel|not))|.attachment.met' "$TR" 2>/dev/null | jq -s length; } || echo 0 )"
  fi
  jq -n --arg state "$state" --arg herdr "$hstate" --arg reason "$reason" --arg result "$result" \
        --argjson verdicts "$verdicts" \
        '{state:$state,herdr_status:$herdr,goal_reason:$reason,goal_result_line:$result,goal_verdicts:$verdicts}'
}

if [[ $WAIT == 1 ]]; then
  START=$(date +%s)
  while :; do
    # block on herdr's server-side wait (event-driven) instead of tight polling; 5-min slices
    # so --kill-after can fire
    # shellcheck disable=SC1010  # "done" is a herdr --until value, not the shell keyword
    H agent wait task --until idle --until done --until blocked --timeout 300000 >/dev/null || true
    S=$(check)
    echo "$S"
    if [[ "$(jq -r .state <<<"$S")" != RUNNING ]]; then break; fi
    if [[ "$KILL_AFTER" -gt 0 && $(( ($(date +%s)-START)/60 )) -ge "$KILL_AFTER" ]]; then
      H agent prompt task '/goal clear' >/dev/null || true
      echo '{"state":"KILLED_TIMEOUT"}'
      break
    fi
  done
else
  check
fi
H pane read "$PANE" --source visible > "$RUN/out/screen.txt" 2>/dev/null || true   # last rendered screen
