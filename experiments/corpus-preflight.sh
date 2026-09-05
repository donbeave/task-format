#!/usr/bin/env bash
# Verify the external proof corpus required for Phase 6.
#
# Usage:
#   bash experiments/corpus-preflight.sh /path/to/pgtui-proof.git
#
# Artifact contract (the argument is a bare or non-bare Git repository):
#
#   refs/tags/taskfmt/TASK-001/baseline
#   refs/tags/taskfmt/TASK-001/reference
#   ... through TASK-007
#
# Every `baseline` and `reference` ref must name a commit.  `reference` must
# descend from its paired `baseline`.  The baseline must already contain the
# package's `trusted/` overlay byte-for-byte; the reference must leave that
# overlay unchanged.  The verifier derives the actual solution patch from
# baseline..reference and runs `taskfmt selfcheck`.  Thus neither an arbitrary
# directory nor a copied test suite can stand in for proof.
#
# A passing run proves, per task, that the supplied baseline is RED for the
# configured focused checks and the supplied reference is GREEN for every gate
# check.  It deliberately does not claim a result when the artifact repository,
# a required ref, its trusted overlay, its ancestry, or a host prerequisite is
# absent.

set -u

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
proof_repo=${1-}

if [ "$#" -ne 1 ]; then
    printf '%s\n' "CORPUS-PREFLIGHT usage: bash experiments/corpus-preflight.sh /path/to/pgtui-proof.git" >&2
    exit 64
fi

if ! git -C "$proof_repo" rev-parse --git-dir >/dev/null 2>&1; then
    printf '%s\n' "CORPUS-PREFLIGHT artifact-repository-missing path=$proof_repo" >&2
    printf '%s\n' "CORPUS-PREFLIGHT require refs/tags/taskfmt/TASK-001..007/{baseline,reference}" >&2
    exit 66
fi

if ! git -C "$proof_repo" fsck --no-dangling >/dev/null 2>&1; then
    printf '%s\n' "CORPUS-PREFLIGHT artifact-repository-invalid path=$proof_repo" >&2
    exit 65
fi

scratch=$(mktemp -d "${TMPDIR:-/tmp}/taskfmt-corpus-preflight.XXXXXX") || exit 70
cleanup() {
    rm -rf "$scratch"
}
trap cleanup EXIT HUP INT TERM

failed=0
for number in 001 002 003 004 005 006 007; do
    task="TASK-$number"
    baseline_ref="refs/tags/taskfmt/$task/baseline"
    reference_ref="refs/tags/taskfmt/$task/reference"
    baseline=$(git -C "$proof_repo" rev-parse --verify --quiet "$baseline_ref^{commit}" 2>/dev/null || true)
    reference=$(git -C "$proof_repo" rev-parse --verify --quiet "$reference_ref^{commit}" 2>/dev/null || true)

    if [ -z "$baseline" ]; then
        printf '%s\n' "CORPUS-PREFLIGHT $task MISSING baseline-ref=$baseline_ref" >&2
        failed=1
        continue
    fi
    if [ -z "$reference" ]; then
        printf '%s\n' "CORPUS-PREFLIGHT $task MISSING reference-ref=$reference_ref" >&2
        failed=1
        continue
    fi
    if ! git -C "$proof_repo" merge-base --is-ancestor "$baseline" "$reference"; then
        printf '%s\n' "CORPUS-PREFLIGHT $task INVALID reference-not-descendant baseline=$baseline reference=$reference" >&2
        failed=1
        continue
    fi

    trusted="$repo_root/experiments/tasks/$task/trusted"
    trusted_ok=1
    while IFS= read -r -d '' source; do
        relative=${source#"$trusted/"}
        base_blob=$(git -C "$proof_repo" rev-parse --verify --quiet "$baseline:$relative" 2>/dev/null || true)
        reference_blob=$(git -C "$proof_repo" rev-parse --verify --quiet "$reference:$relative" 2>/dev/null || true)
        baseline_copy=$(mktemp "$scratch/trusted-baseline.XXXXXX")
        reference_copy=$(mktemp "$scratch/trusted-reference.XXXXXX")
        if ! git -C "$proof_repo" show "$baseline:$relative" >"$baseline_copy" 2>/dev/null \
            || ! git -C "$proof_repo" show "$reference:$relative" >"$reference_copy" 2>/dev/null \
            || ! cmp -s "$source" "$baseline_copy" \
            || ! cmp -s "$source" "$reference_copy"; then
            printf '%s\n' "CORPUS-PREFLIGHT $task INVALID trusted-overlay path=$relative baseline=$base_blob reference=$reference_blob" >&2
            trusted_ok=0
        fi
    done < <(find "$trusted" -type f -print0)
    if [ "$trusted_ok" -ne 1 ]; then
        failed=1
        continue
    fi

    work="$scratch/$task-work"
    patch="$scratch/$task-reference.patch"
    if ! git -C "$proof_repo" worktree add --detach --quiet "$work" "$baseline" \
        || ! git -C "$proof_repo" diff --binary "$baseline..$reference" >"$patch"; then
        printf '%s\n' "CORPUS-PREFLIGHT $task INTERNAL checkout-or-patch-failed" >&2
        failed=1
        continue
    fi
    if rtk cargo run --quiet --manifest-path "$repo_root/harness/Cargo.toml" -- \
        selfcheck "$repo_root/experiments/tasks/$task" "$work" --base "$baseline" --reference "$patch"; then
        printf '%s\n' "CORPUS-PREFLIGHT $task PASS baseline=$baseline reference=$reference"
    else
        printf '%s\n' "CORPUS-PREFLIGHT $task FAIL baseline=$baseline reference=$reference" >&2
        failed=1
    fi
    git -C "$proof_repo" worktree remove --force "$work" >/dev/null 2>&1 || true
done

if [ "$failed" -ne 0 ]; then
    printf '%s\n' "CORPUS-PREFLIGHT RESULT FAIL"
    exit 1
fi
printf '%s\n' "CORPUS-PREFLIGHT RESULT PASS"
