#!/usr/bin/env bash
# manifest.sh — generate / check the protected-path sha256 manifest at dispatch time.
#   manifest.sh gen   [-o protected.sha256] [-r ROOT] PATH...   # PATH = file or dir (recursed); relative to ROOT
#   manifest.sh check [-m protected.sha256] [-r ROOT]
# Line format: "<sha256>  <root-relative path>", sorted by path — compatible with `sha256sum -c`.
set -Eeuo pipefail
sha256_cmd() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$@"; else shasum -a 256 "$@"; fi; }

mode="${1:-}"; shift || true
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"; OUT="protected.sha256"; MAN="protected.sha256"
while getopts "o:r:m:" opt; do case "$opt" in o) OUT="$OPTARG";; r) ROOT="$OPTARG";; m) MAN="$OPTARG";; *) exit 64;; esac; done
shift $((OPTIND-1))
OUT="$(cd "$(dirname "$OUT")" && pwd)/$(basename "$OUT")"
MAN="$(cd "$(dirname "$MAN")" 2>/dev/null && pwd)/$(basename "$MAN")"
cd "$ROOT"

case "$mode" in
  gen)
    [[ $# -gt 0 ]] || { echo "usage: manifest.sh gen [-o OUT] [-r ROOT] PATH..." >&2; exit 64; }
    tmp="$(mktemp)"
    for p in "$@"; do
      p="${p#./}"
      if [[ -d "$p" ]]; then find "$p" -type f -print
      elif [[ -f "$p" ]]; then printf '%s\n' "$p"
      else echo "no such path: $p" >&2; exit 66; fi
    done | LC_ALL=C sort -u | while IFS= read -r f; do sha256_cmd "$f"; done >"$tmp"
    sed -E 's/^([0-9a-f]{64}) [ *]?(\.\/)?/\1  /' "$tmp" | LC_ALL=C sort -k2 >"$OUT"
    rm -f "$tmp"
    printf 'MANIFEST %s entries=%s\n' "$OUT" "$(wc -l <"$OUT" | tr -d ' ')"
    ;;
  check)
    rc=0
    while IFS= read -r line; do
      [[ -z "$line" || "$line" == \#* ]] && continue
      want="${line%% *}"; path="${line#* }"; path="${path# }"
      if [[ ! -f "$path" ]]; then echo "MISSING  $path"; rc=1; continue; fi
      have="$(sha256_cmd "$path" | awk '{print $1}')"
      if [[ "$have" == "$want" ]]; then echo "ok       $path"; else echo "MODIFIED $path"; rc=1; fi
    done <"$MAN"
    exit $rc
    ;;
  *) echo "usage: manifest.sh gen|check ..." >&2; exit 64;;
esac
