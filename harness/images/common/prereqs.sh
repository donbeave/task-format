#!/usr/bin/env bash
# prereqs.sh — task-container runtime prerequisites: postgres 16 (inner docker) + seed restore.
#
#   (no args; run as root from entrypoint.sh)
#
# Loads the pre-baked postgres tarball, starts prereq-postgres on 127.0.0.1:5432, restores every
# seed file (fixture seeds first, then /seed/*.sql). Exit 0 → /out/prereqs.json {ok:true,...} +
# /out/prereqs.ready. Exit !=0 → entrypoint.sh parks the container (never exits on failure).
# No seed file at all is fine: postgres up = prerequisite met.
set -Eeuo pipefail

PG_IMAGE=postgres:16-alpine
PG_CONTAINER=prereq-postgres
PG_USER=pgtui
PG_PASSWORD=pgtui
PG_DB=pgtui
TAR=/opt/preload/postgres.tar

exec > >(tee -a /out/prereqs.log) 2>&1
echo "=== prereqs start $(date -u +%FT%TZ) ==="

# 1. baked image tarball (built by harness/preload.sh on the host)
if [ ! -f "$TAR" ]; then
  echo "ERROR: $TAR missing — run harness/preload.sh first"
  exit 1
fi
docker load -i "$TAR"

# 2. postgres on loopback only
docker rm -f "$PG_CONTAINER" >/dev/null 2>&1 || true   # idempotent on container restart
docker run -d --name "$PG_CONTAINER" -p 127.0.0.1:5432:5432 \
      -e POSTGRES_USER=$PG_USER -e POSTGRES_PASSWORD=$PG_PASSWORD -e POSTGRES_DB=$PG_DB \
      "$PG_IMAGE"
for _ in $(seq 1 60); do pg_isready -h 127.0.0.1 -p 5432 -U "$PG_USER" && break; sleep 1; done
pg_isready -h 127.0.0.1 -p 5432 -U "$PG_USER"

# 3. seeds: /work/**/tests/fixtures/seed.sql (sorted), then /seed/*.sql (sorted) — ON_ERROR_STOP
mapfile -d '' SEEDS < <(find /work -type f -path '*/tests/fixtures/seed.sql' -print0 | LC_ALL=C sort -z)
if [ -d /seed ]; then
  mapfile -d '' -O "${#SEEDS[@]}" SEEDS < <(find /seed -maxdepth 1 -type f -name '*.sql' -print0 | LC_ALL=C sort -z)
fi
if [ "${#SEEDS[@]}" -eq 0 ]; then
  echo "WARN: no seed files found — continuing (postgres up = prerequisite met)"
fi
seeds_json='[]'
for f in "${SEEDS[@]}"; do
  echo "restore seed: $f"
  docker exec -i "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -v ON_ERROR_STOP=1 <"$f"
  seeds_json="$(jq -c --arg f "$f" '. + [$f]' <<<"$seeds_json")"
done

# 4. success markers
jq -n --arg dsn "postgres://$PG_USER:$PG_PASSWORD@127.0.0.1:5432/$PG_DB" \
      --arg container "$PG_CONTAINER" --arg image "$PG_IMAGE" \
      --argjson seeds "$seeds_json" --arg finished "$(date -u +%FT%TZ)" \
      '{ok:true,
        postgres_dsn:$dsn,
        postgres_container:$container,
        image:$image,
        seeds:$seeds,
        finished:$finished}' >/out/prereqs.json
touch /out/prereqs.ready
echo "=== prereqs ok: $PG_IMAGE up, ${#SEEDS[@]} seed(s) restored ==="
