#!/bin/sh
# A cargo success is insufficient: disabled/unavailable Docker reports SKIP. Release success
# requires this invocation's complete proof instead.
set -eu

proof_dir=$(mktemp -d "${TMPDIR:-/tmp}/taskfmt-docker-itest.XXXXXX")
trap 'rm -rf "$proof_dir"' EXIT HUP INT TERM
proof="$proof_dir/proof.json"

TASKFMT_ITEST_DOCKER=1 TASKFMT_ITEST_PROOF="$proof" \
  cargo test --manifest-path harness/Cargo.toml --test docker_itest

expected='{"schema":"taskfmt/docker-itest-proof/v1","enabled":true,"checks":[{"id":"container_runs_prereq_stage_and_stays_up","result":"PASS"},{"id":"arch_detection_agrees_with_the_server","result":"PASS"}]}'
if [ ! -f "$proof" ]; then
  echo "docker_itest: FAIL enabled end-to-end bodies did not execute (runner reported SKIP or failed before proof)" >&2
  exit 1
fi
actual=$(tr -d '\n' < "$proof")
if [ "$actual" != "$expected" ]; then
  echo "docker_itest: FAIL incomplete or invalid enabled-body proof" >&2
  exit 1
fi
echo "docker_itest: PASS enabled bodies proved"
