#!/usr/bin/env bash
# build.sh — build the harness images: harness-base, then harness-claude / harness-codex on top.
#
#   build.sh                    # all three, pinned versions from the Dockerfiles
#   CLAUDE_CODE_VERSION=x build.sh   /  CODEX_VERSION=x build.sh   # override an agent CLI pin
#
# Build context is harness/ (the Dockerfiles COPY images/common/* and images/codex/*).
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# TARGETARCH for the herdr release asset; plain `docker build` does not set it (buildx would)
ARCH="$(docker version --format '{{.Server.Arch}}' 2>/dev/null || true)"
[ -n "$ARCH" ] || ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|amd64)  TARGETARCH=amd64 ;;
  aarch64|arm64) TARGETARCH=arm64 ;;
  *) echo "build.sh: unsupported architecture: $ARCH" >&2; exit 1 ;;
esac
echo "== target arch: $TARGETARCH (${ARCH})"

# prereqs.sh docker-loads the postgres tarball baked into harness-base (COPY images/preload/...)
if [ ! -s "$ROOT/images/preload/postgres.tar" ]; then
  echo "build.sh: $ROOT/images/preload/postgres.tar missing — run harness/preload.sh first" >&2
  exit 1
fi

echo "== harness-base"
docker build --progress=plain -f "$ROOT/images/base/Dockerfile" \
      --build-arg TARGETARCH="$TARGETARCH" -t harness-base:latest "$ROOT"

echo "== harness-claude (claude-code ${CLAUDE_CODE_VERSION:-2.1.250})"
docker build --progress=plain -f "$ROOT/images/claude/Dockerfile" \
      --build-arg TARGETARCH="$TARGETARCH" \
      --build-arg CLAUDE_CODE_VERSION="${CLAUDE_CODE_VERSION:-2.1.250}" \
      -t harness-claude:latest "$ROOT"

echo "== harness-codex (codex ${CODEX_VERSION:-0.150.1})"
docker build --progress=plain -f "$ROOT/images/codex/Dockerfile" \
      --build-arg TARGETARCH="$TARGETARCH" \
      --build-arg CODEX_VERSION="${CODEX_VERSION:-0.150.1}" \
      -t harness-codex:latest "$ROOT"

echo "== built: harness-base:latest harness-claude:latest harness-codex:latest"
