#!/usr/bin/env bash
# run-headed.sh — dispatch one task package to one agent TUI in a persistent Docker container.
#
#   run-headed.sh <claude|codex> <TASK-ID> [--model M] [--effort E] [--net api|all] [--seed-dir DIR]
#
# Defaults: --effort high; --model sonnet (claude) / image default (codex); --net all;
#   --seed-dir ../experiments/fixtures/seed.
# The container runs detached, --privileged (inner dockerd for testcontainers, D19), NO --rm:
#   the operator re-attaches later (attach.sh) and inspects (status.sh).
# The entrypoint brings up the prereq stage first (inner dockerd on vfs, docker load of the
#   preloaded postgres:16-alpine, standing prereq-postgres on 127.0.0.1:5432, seed restore from
#   /work/**/tests/fixtures/seed.sql then /seed/*.sql) and only then launches the agent under
#   herdr. prereqs.ready / prereqs.FAILED (+ prereqs.json, prereqs.log) land in /out; on FAILED
#   this script dumps them, prints the container name and exits 1 — container left up.
set -Eeuo pipefail

usage() { echo "usage: run-headed.sh <claude|codex> <TASK-ID> [--model M] [--effort E] [--net api|all] [--seed-dir DIR]" >&2; }

[[ $# -ge 2 ]] || { usage; exit 64; }
AGENT=$1; TASK=$2; shift 2
MODEL=""; EFFORT="high"; NET_MODE="all"; SEED_DIR=""
while [[ $# -gt 0 ]]; do case $1 in
  --model)    MODEL=$2;    shift 2;;
  --effort)   EFFORT=$2;   shift 2;;
  --net)      NET_MODE=$2; shift 2;;
  --seed-dir) SEED_DIR=$2; shift 2;;
  *) echo "bad arg $1" >&2; usage; exit 64;;
esac; done
case $AGENT in claude|codex) ;; *) echo "agent must be claude or codex" >&2; exit 64;; esac
# API-only firewall is deferred (D21): the inner dockerd shares the outer netns, so an outer
# default-DROP iptables ruleset breaks DinD. v1 always runs NET_MODE=all.
[[ "$NET_MODE" == all ]] || { echo "--net api is not wired yet (firewall deferred, see RESEARCH-FINDINGS.md); v1 runs --net all" >&2; exit 2; }

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXPERIMENTS="$ROOT/../experiments"
TEMPLATE="$ROOT/../reference/task-template"

TASK_DIR="$EXPERIMENTS/tasks/$TASK"
[[ -d "$TASK_DIR" ]] || { echo "no such task dir: $TASK_DIR" >&2; exit 66; }
FIXTURE_NAME="$(cat "$TASK_DIR/fixture")"
FIXTURE="$EXPERIMENTS/fixtures/$FIXTURE_NAME"
[[ -n "$SEED_DIR" ]] || SEED_DIR="$EXPERIMENTS/fixtures/seed"
if [[ "$SEED_DIR" != "$EXPERIMENTS/fixtures/seed" && ! -d "$SEED_DIR" ]]; then
  echo "seed dir not found: $SEED_DIR" >&2; exit 66
fi

RUN_ID="$(date -u +%Y%m%d-%H%M%S)-$AGENT-$TASK"
RUN="$EXPERIMENTS/runs/$RUN_ID"
CNAME="harness-$RUN_ID"
IMAGE="harness-$AGENT:latest"
mkdir -p "$RUN"/{workspace,agent-home,out,task-snapshot,progress}

# ---------- 1. inputs ----------
cp -a "$TASK_DIR"/. "$RUN/task-snapshot/"
# dispatch top-up: task dirs hold the authored files; the shared protocol + gate come from the template
for f in AGENTS.md verify.sh; do
  [[ -e "$RUN/task-snapshot/$f" ]] || cp "$TEMPLATE/$f" "$RUN/task-snapshot/$f"
done
[[ -e "$RUN/task-snapshot/CLAUDE.md" ]] || ln -s AGENTS.md "$RUN/task-snapshot/CLAUDE.md"

if [[ -d "$FIXTURE" ]]; then
  cp -a "$FIXTURE"/. "$RUN/workspace/"
  rm -rf "$RUN/workspace/.git"     # the harness owns the baseline commit, not the fixture
else
  echo "note: fixture '$FIXTURE_NAME' not built yet — empty workspace (smoke run)" >&2
fi

if ! "$ROOT/task-lint.sh" "$TASK_DIR" >"$RUN/lint.log" 2>&1; then
  echo "task-lint FAILED — not dispatching (log: $RUN/lint.log)" >&2; cat "$RUN/lint.log" >&2; exit 1
fi
"$ROOT/progress-init.sh" "$TASK_DIR" -o "$RUN/progress/progress.md"
( cd "$RUN/workspace" \
  && git init -q && git add -A \
  && git -c user.name=harness -c user.email=harness@localhost commit -q --allow-empty -m baseline --no-verify \
  && git tag baseline )

# ---------- 2. prompt + session ----------
# the /goal block of goal-prompt.md collapsed to one line (agent prompt handles multi-line via
# bracketed paste, but one line avoids the [Pasted text] chip)
PROMPT="$(awk '/^```text/{f=1;next} /^```/{f=0} f' "$ROOT/goal-prompt.md" | tr '\n' ' ' | sed 's/  */ /g; s/ $//')"
[[ -n "$PROMPT" ]] || { echo "no prompt block found in $ROOT/goal-prompt.md" >&2; exit 1; }
printf '%s\n' "$PROMPT" > "$RUN/prompt.txt"
SESSION_ID="$(uuidgen | tr '[:upper:]' '[:lower:]')"

# ---------- 3. agent home pre-seed + command line ----------
MOUNTS=(-v "$RUN/workspace:/work" -v "$RUN/task-snapshot:/task:ro" -v "$RUN/progress:/progress"
        -v "$RUN/agent-home:/agent-home" -v "$RUN/out:/out")
if [[ -d "$SEED_DIR" ]]; then
  cp -a "$SEED_DIR" "$RUN/seed"
  MOUNTS+=(-v "$RUN/seed:/seed:ro")
else
  echo "note: no seed dir at $SEED_DIR — prereq-postgres starts unseeded" >&2
fi

case $AGENT in
  claude)
    [[ -n "${ANTHROPIC_API_KEY:-}" ]] || { echo "ANTHROPIC_API_KEY not set" >&2; exit 1; }
    MODEL="${MODEL:-sonnet}"
    cat > "$RUN/agent-home/settings.json" <<EOF
{"skipDangerousModePermissionPrompt": true,
 "theme": "dark",
 "tui": "default",
 "cleanupPeriodDays": 3650,
 "env": {"CLAUDE_CODE_GOAL_CHECKIN_MINUTES": "0"}}
EOF
    KEY_TAIL="${ANTHROPIC_API_KEY: -20}"
    cat > "$RUN/agent-home/.claude.json" <<EOF
{"hasCompletedOnboarding": true,
 "lastOnboardingVersion": "2.1.250",
 "numStartups": 1,
 "projects": {"/work": {"hasTrustDialogAccepted": true, "hasCompletedProjectOnboarding": true, "allowedTools": []}},
 "customApiKeyResponses": {"approved": ["$KEY_TAIL"], "rejected": []}}
EOF
    AGENT_CMD="claude --dangerously-skip-permissions --session-id $SESSION_ID --add-dir /task --add-dir /progress --model $MODEL --effort $EFFORT"
    ENVS=(-e ANTHROPIC_API_KEY -e CLAUDE_CONFIG_DIR=/agent-home -e CLAUDE_CODE_PROJECT_DIR_NAME=work
          -e CLAUDE_CODE_EFFORT_LEVEL="$EFFORT")
    ;;
  codex)
    [[ -n "${OPENAI_API_KEY:-}" ]] || { echo "OPENAI_API_KEY not set" >&2; exit 1; }
    cat > "$RUN/agent-home/config.toml" <<'EOF'
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
EOF
    # codex login --with-api-key runs in the entrypoint (OPENAI_API_KEY passed through below)
    AGENT_CMD="codex --dangerously-bypass-approvals-and-sandbox --no-alt-screen -C /work --add-dir /task --add-dir /progress${MODEL:+ -m $MODEL} -c model_reasoning_effort=\"$EFFORT\""
    ENVS=(-e OPENAI_API_KEY -e CODEX_HOME=/agent-home)
    ;;
esac

# ---------- 4. container: named, persistent, detached, no -t (herdr server needs no TTY) ----------
docker run -d --privileged --name "$CNAME" \
  -e NET_MODE="$NET_MODE" -e AGENT_CMD="$AGENT_CMD" -e AGENT_KIND="$AGENT" -e HERDR_SESSION=agent \
  "${ENVS[@]}" "${MOUNTS[@]}" \
  --memory 4g --cpus 2 --pids-limit 2048 "$IMAGE" >/dev/null

H() { docker exec -u agent "$CNAME" herdr "$@"; }     # HERDR_SESSION=agent resolves the socket

# ---------- 5. prereq stage: dockerd (vfs) + docker load + prereq-postgres + seeds ----------
echo "container $CNAME up; waiting for prereq stage (inner dockerd + postgres, up to 180 s)..."
fail_prereqs() {
  echo "prereq stage FAILED — container $CNAME left up for inspection" >&2
  echo "--- $RUN/out/prereqs.json ---"; cat "$RUN/out/prereqs.json" 2>/dev/null || true
  echo "--- $RUN/out/prereqs.log ---";  cat "$RUN/out/prereqs.log" 2>/dev/null || true
  exit 1
}
READY=0
for _ in $(seq 1 180); do
  if [[ -f "$RUN/out/prereqs.FAILED" ]]; then fail_prereqs; fi
  if [[ -f "$RUN/out/prereqs.ready" ]]; then READY=1; break; fi
  sleep 1
done
if [[ $READY -ne 1 ]]; then echo "prereqs not ready after 180 s" >&2; fail_prereqs; fi
echo "prereqs ready:"; cat "$RUN/out/prereqs.json" 2>/dev/null || true

# ---------- 6. herdr pane + meta ----------
PANE=""
for _ in $(seq 1 100); do
  if [[ -s "$RUN/out/pane-id" ]]; then PANE="$(cat "$RUN/out/pane-id")"; break; fi
  sleep 0.5
done
[[ -n "$PANE" ]] || { echo "no /out/pane-id after 50 s — container $CNAME left up (docker logs $CNAME)" >&2; exit 1; }

TR="$RUN/agent-home/projects/work/$SESSION_ID.jsonl"
jq -n --arg run "$RUN_ID" --arg container "$CNAME" --arg agent "$AGENT" --arg task "$TASK" \
      --arg model "$MODEL" --arg effort "$EFFORT" --arg net "$NET_MODE" \
      --arg session "$SESSION_ID" --arg pane "$PANE" --arg start "$(date -u +%FT%TZ)" \
      '{run:$run,container:$container,agent:$agent,task:$task,model:$model,effort:$effort,net:$net,
        session_id:$session,pane:$pane,start:$start}' > "$RUN/meta.json"

# ---------- 7. readiness: herdr detects the agent (HERDR_AGENT hint) and classifies it idle ----------
if ! H agent wait "$PANE" --until idle --timeout 180000 >/dev/null 2>"$RUN/out/wait-ready.err"; then
  echo "agent not idle after 180 s (dialog? auth?). Screen:" >&2
  H pane read "$PANE" --source visible >&2 || true
  cat "$RUN/out/wait-ready.err" >&2 || true
  exit 1
fi
H agent rename "$PANE" task >/dev/null                # stable target name for attach.sh/status.sh

# ---------- 8. inject the /goal prompt (text + Enter; refuses if a dialog is up) ----------
if ! H agent prompt task "$PROMPT" >"$RUN/out/prompt.json" 2>"$RUN/out/prompt.err"; then
  echo "prompt refused:" >&2; cat "$RUN/out/prompt.err" >&2 || true
  H pane read "$PANE" --source visible >&2 || true
  exit 1
fi

# confirm the goal was accepted: transcript sentinel within ~30 s; codex: agent starts working.
# Enter eaten by the slash-command popup? (UNVERIFIED failure mode) — resend once at i=5.
ACCEPTED=0
for i in $(seq 1 15); do
  sleep 2
  if [[ "$AGENT" == claude ]] && grep -q '"goal_status"' "$TR" 2>/dev/null; then
    echo "goal accepted (transcript sentinel)"; ACCEPTED=1; break
  fi
  if [[ "$AGENT" == codex ]] && [[ "$(H agent get task | jq -r '.result.agent.agent_status // ""' 2>/dev/null || true)" == working ]]; then
    echo "prompt consumed (agent working)"; ACCEPTED=1; break
  fi
  if [[ $i -eq 5 ]] && H pane read "$PANE" --source visible 2>/dev/null | grep -qF "${PROMPT:0:40}"; then
    echo "prompt visible but not submitted — resending Enter" >&2
    H agent send-keys task enter >/dev/null 2>&1 || true
  fi
done
if [[ $ACCEPTED -ne 1 ]]; then
  echo "warning: goal acceptance not confirmed within 30 s — attach and check: $ROOT/attach.sh $RUN_ID" >&2
fi

# ---------- 9. summary ----------
case $AGENT in
  claude) TR_DISPLAY="$TR";;
  codex)  TR_DISPLAY="$RUN/agent-home/sessions/**/rollout-*.jsonl";;
esac
cat <<EOF
run:        $RUN
container:  $CNAME   (persistent; docker rm -f $CNAME when done)
attach:     $ROOT/attach.sh $RUN_ID   (detach: ctrl+b q — never ctrl+c)
status:     $ROOT/status.sh $RUN_ID [--wait]
raw log:    $RUN/out/tui.log           (script(1) stream, from the first byte)
transcript: $TR_DISPLAY
prereqs:    $RUN/out/prereqs.json
progress:   $RUN/progress/progress.md
EOF
