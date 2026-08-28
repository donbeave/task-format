#!/usr/bin/env bash
# preload.sh — host-side: bake the postgres image tarball the prereq layer loads at container start.
#
#   preload.sh
#
# Pulls postgres:16-alpine (pinned by digest when images/preload/postgres.digest exists), records
# the digest (committed) and saves the tarball next to it (gitignored — regenerate on demand, the
# digest pin makes it deterministic).
set -Eeuo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PRELOAD="$ROOT/images/preload"
DIGEST_FILE="$PRELOAD/postgres.digest"
TAR="$PRELOAD/postgres.tar"
REF=postgres:16-alpine

mkdir -p "$PRELOAD"
if [ -s "$DIGEST_FILE" ]; then
  PINNED="$(cat "$DIGEST_FILE")"
  echo "== pull pinned: $PINNED"
  docker pull "$PINNED"
  docker tag "$PINNED" "$REF"
else
  echo "== no digest pin yet — pulling $REF (this run records the digest)"
  docker pull "$REF"
  docker image inspect --format '{{index .RepoDigests 0}}' "$REF" >"$DIGEST_FILE"
fi
echo "== digest: $(cat "$DIGEST_FILE")"

echo "== save $REF -> $TAR"
docker save "$REF" -o "$TAR"
echo "== wrote $TAR ($(du -h "$TAR" | cut -f1)); prereqs.sh will docker load it"
