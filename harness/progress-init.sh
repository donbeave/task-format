#!/usr/bin/env bash
# progress-init.sh — generate the initial progress.md for one task package from its README.md.
#
#   progress-init.sh <task-dir | README.md> [-o progress.md]
#
# progress.md is never stored in the repository: it is derived from README.md at dispatch
# (fenced header + verbatim checklist block + empty Log + Handoff) and mounted read-write at
# /progress/progress.md. The header sits between --- fences so markdown viewers render it.
# verify.sh later diffs its checklist block against README.md.
# The README.md is linted first; an invalid contract never yields a progress file.
set -Eeuo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

target="${1:-.}"; shift || true
OUT="-"
while getopts "o:" opt; do case "$opt" in o) OUT="$OPTARG";; *) exit 64;; esac; done
if [[ -d "$target" ]]; then README="$target/README.md"; else README="$target"; fi
[[ -f "$README" ]] || { echo "progress-init: missing $README" >&2; exit 66; }

lint_out="$("$SCRIPT_DIR/task-lint.sh" "$README" 2>&1)" || { printf '%s\n' "$lint_out" >&2; echo "progress-init: README.md failed task-lint.sh; not generating" >&2; exit 65; }

id="$(awk 'NR==1 && $0!="---"{exit} NR==1{next} /^---$/{exit} {print}' "$README" | sed -nE 's/^id: *"?(TASK-[0-9]+)"?.*/\1/p' | head -1)"
checklist="$(awk '/<!-- checklist:start -->/{f=1;next} /<!-- checklist:end -->/{f=0} f' "$README")"
# first leaf = first item whose successor is not deeper (or the last item)
first_leaf="$(printf '%s\n' "$checklist" | awk '
  function depth(s,  n){ n=match(s,/[^ ]/); return (n-1)/4 }
  { cnt++; d[cnt]=depth($0); id=$0; match(id,/\*\*[0-9.]+\*\*/); ids[cnt]=substr(id,RSTART+2,RLENGTH-4) }
  END { for (i=1;i<=cnt;i++) if (i==cnt || d[i+1]<=d[i]) { print ids[i]; exit } }')"

body="$(cat <<EOT
---
TASK: $id
STATE: IN_PROGRESS
CURRENT: $first_leaf
BASELINE: <not run>
---

<!-- checklist:start -->
$checklist
<!-- checklist:end -->

## Log

## Handoff
NEXT: $first_leaf
CURRENT_FAILURE: none
DECISIONS: none
EOT
)"
if [[ "$OUT" == "-" ]]; then printf '%s\n' "$body"; else printf '%s\n' "$body" >"$OUT"; echo "PROGRESS $OUT task=$id current=$first_leaf" >&2; fi
