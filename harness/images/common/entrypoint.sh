#!/usr/bin/env bash
# entrypoint.sh — container PID 1 (root): inner dockerd → prerequisites → agent (as `agent`).
#
#   (no args; configured by env)
#     DOCKERD_STORAGE_DRIVER   inner daemon storage driver (default vfs — works on any backing fs)
#     PREREQ_ONLY=1            test mode: stop after prerequisites, no agent
#
# HARD RULE: never exit on prerequisite failure — park (park(), never returns) so the operator
# can re-attach and debug the live container. Markers in /out: prereqs.json, prereqs.ready,
# prereqs.FAILED, prereqs.log, dockerd.log.
set -Eeuo pipefail

park() {  # stay alive, debuggable; SIGTERM → clean exit 0. Never returns.
  trap 'exit 0' TERM INT
  while :; do sleep 86400 & wait $! || true; done
}

# (a) inner Docker daemon (DinD). dockerd runs in the foreground — background it ourselves;
#     the prereq layer runs postgres inside it. A docker stop that escalates to SIGKILL leaves
#     a stale /var/run/docker.pid behind which blocks the daemon on the next boot — drop it.
rm -f /var/run/docker.pid
dockerd --data-root /var/lib/docker --storage-driver "${DOCKERD_STORAGE_DRIVER:-vfs}" \
        --host unix:///var/run/docker.sock >/out/dockerd.log 2>&1 &
for _ in $(seq 1 60); do docker info >/dev/null 2>&1 && break; sleep 1; done
docker info >/dev/null 2>&1 \
  || echo "entrypoint: dockerd not ready after 60s — see /out/dockerd.log; prerequisites will fail" >&2
chmod 666 /var/run/docker.sock || true

# codex image: seed $CODEX_HOME/config.toml from the baked copy (a fresh run mounts an empty dir)
if [ -n "${CODEX_HOME:-}" ]; then
  mkdir -p "$CODEX_HOME"
  if [ -f /etc/codex-config.toml ] && [ ! -f "$CODEX_HOME/config.toml" ]; then
    cp /etc/codex-config.toml "$CODEX_HOME/config.toml"
  fi
  chown -R agent "$CODEX_HOME" 2>/dev/null || true
  # run-headed.sh passes OPENAI_API_KEY through; auth.json must exist before the agent starts
  if [ -n "${OPENAI_API_KEY:-}" ]; then
    gosu agent bash -c 'printenv OPENAI_API_KEY | codex login --with-api-key' \
      || echo "entrypoint: codex login failed — check OPENAI_API_KEY" >&2
  fi
fi
chown agent /work /out /agent-home 2>/dev/null || true

# (b) prerequisites (postgres + seeds). On failure: markers + park — the container must stay up.
#     Stale markers from a previous boot (docker stop/start) must not survive the re-run.
rm -f /out/prereqs.ready /out/prereqs.FAILED
set +e
/usr/local/bin/prereqs.sh
PREREQ_RC=$?
set -e
if [ "$PREREQ_RC" -ne 0 ]; then
  touch /out/prereqs.FAILED
  sleep 1  # let the tee inside prereqs.sh flush the log before we read it
  jq -n --arg error "$(tail -n 20 /out/prereqs.log 2>/dev/null | tr '\n' ' ' | cut -c1-2000)" \
        '{ok:false, error:$error}' >/out/prereqs.json
  echo "entrypoint: prerequisites failed (rc=$PREREQ_RC) — parking; inspect /out/prereqs.log" >&2
  park
fi

# (d) test mode: prerequisites only (prereqs.ready written by prereqs.sh), no agent
if [ "${PREREQ_ONLY:-0}" = 1 ]; then
  echo "entrypoint: PREREQ_ONLY=1 — prerequisites done, parking without agent" >&2
  park
fi

# (c) agent supervision as user `agent`; TERM/INT handled there (graceful herdr server stop)
exec gosu agent /usr/local/bin/agent-launch.sh
