#!/usr/bin/env bash
# verify.sh — generic completion gate for one task package.
#
# Contract: exit 0 AND last stdout line exactly "DONE"  <=>  every check passed.
# All checks run (no short-circuit) so the executor gets complete feedback.
#
# Executor (inside container, from the repo root):   /task/verify.sh
# Outer gate (host, trusted copies):
#   VERIFY_ROOT=<workspace> VERIFY_TASK_DIR=<task-snapshot> PROGRESS_FILE=<run>/progress.md \
#     <task-snapshot>/verify.sh
#
# Env (all optional):
#   VERIFY_ROOT       repo root (default: git toplevel of cwd, else cwd)
#   VERIFY_TASK_DIR   dir holding task.md, verify.config, protected.sha256 (default: dir of this script)
#   PROGRESS_FILE     progress file (default: /progress/progress.md); empty string disables the check
#   VERIFY_LOG_DIR    per-check logs (default: mktemp -d)
#   VERIFY_FAIL_FAST  1 = stop at first failing check
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERIFY_ROOT="${VERIFY_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
VERIFY_TASK_DIR="${VERIFY_TASK_DIR:-$SCRIPT_DIR}"
VERIFY_CONFIG="$VERIFY_TASK_DIR/verify.config"
VERIFY_MANIFEST="$VERIFY_TASK_DIR/protected.sha256"
TASK_FILE="$VERIFY_TASK_DIR/task.md"
PROGRESS_FILE="${PROGRESS_FILE-/progress/progress.md}"
VERIFY_LOG_DIR="${VERIFY_LOG_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/verify.XXXXXX")}"
VERIFY_FAIL_FAST="${VERIFY_FAIL_FAST:-0}"
mkdir -p "$VERIFY_LOG_DIR"
cd "$VERIFY_ROOT"

# Any uncaught error => explicit FAIL, never a silent DONE.
trap 'rc=$?; printf "RESULT FAIL internal-error line=%s rc=%s\n" "$LINENO" "$rc" >&2; printf "RESULT FAIL\n"; exit 70' ERR

# ---------- config defaults (overridden by verify.config) ----------
BASE_REF="${BASE_REF:-}"
FOCUSED_CMDS=(); REGRESSION_CMDS=(); LINT_CMDS=()
FORBIDDEN_PATTERNS=(); FORBIDDEN_PATHS=(); REQUIRED_PATHS=()
ALLOWED_GLOBS=(); DENIED_GLOBS=(); EXTRA_CHECKS=()

if [[ -f "$VERIFY_CONFIG" ]]; then
  # shellcheck disable=SC1090
  source "$VERIFY_CONFIG"
else
  printf 'CHECK config FAIL missing %s\n' "$VERIFY_CONFIG"; printf 'RESULT FAIL\n'; exit 2
fi
printf 'CHECK config PASS %s\n' "$VERIFY_CONFIG"

# ---------- helpers ----------
FAILS=0; PASSES=0
sha256_cmd() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$@"; else shasum -a 256 "$@"; fi; }

run_check() {  # run_check NAME CMD...
  local name="$1"; shift
  local log="$VERIFY_LOG_DIR/$name.log" rc=0
  ( set -Eeuo pipefail; "$@" ) >"$log" 2>&1 || rc=$?
  if [[ $rc -eq 0 ]]; then
    printf 'CHECK %s PASS\n' "$name"; PASSES=$((PASSES+1))
  else
    printf 'CHECK %s FAIL rc=%s log=%s\n' "$name" "$rc" "$log"; FAILS=$((FAILS+1))
    printf -- '--- tail %s ---\n' "$log"; tail -n 40 "$log" | sed 's/^/  | /'; printf -- '---\n'
    if [[ "$VERIFY_FAIL_FAST" == "1" ]]; then finish; fi
  fi
}
run_cmd_list() {  # run_cmd_list PREFIX ARRAYNAME
  local prefix="$1" i=0 cmd; local -n ref="$2"
  for cmd in "${ref[@]}"; do i=$((i+1)); run_check "$prefix.$i" bash -o pipefail -c "$cmd"; done
}
finish() {
  printf 'SUMMARY pass=%s fail=%s log_dir=%s\n' "$PASSES" "$FAILS" "$VERIFY_LOG_DIR"
  if [[ $FAILS -eq 0 ]]; then printf 'RESULT PASS\nDONE\n'; exit 0; fi
  printf 'RESULT FAIL\n'; exit 1
}

# ---------- built-in checks ----------
check_protected_manifest() {
  [[ -f "$VERIFY_MANIFEST" ]] || { echo "manifest missing: $VERIFY_MANIFEST"; return 1; }
  local rc=0 line want path have
  while IFS= read -r line; do
    [[ -z "$line" || "$line" == \#* ]] && continue
    want="${line%% *}"; path="${line#* }"; path="${path# }"
    if [[ ! -f "$path" ]]; then echo "MISSING $path"; rc=1; continue; fi
    have="$(sha256_cmd "$path" | awk '{print $1}')"
    if [[ "$have" != "$want" ]]; then echo "MODIFIED $path"; rc=1; else echo "ok $path"; fi
  done <"$VERIFY_MANIFEST"
  return $rc
}

changed_files() {
  { git diff --name-only "$BASE_REF" --; git diff --name-only --cached --; git ls-files --others --exclude-standard; } \
    | sed '/^$/d' | sort -u
}
matches_any() { local f="$1" g; local -n globs="$2"; for g in "${globs[@]}"; do [[ "$f" == $g ]] && return 0; done; return 1; }
check_scope() {
  [[ -n "$BASE_REF" ]] || { echo "BASE_REF not set"; return 1; }
  git rev-parse --verify --quiet "$BASE_REF^{commit}" >/dev/null || { echo "BASE_REF not resolvable: $BASE_REF"; return 1; }
  [[ ${#ALLOWED_GLOBS[@]} -gt 0 ]] || { echo "ALLOWED_GLOBS empty"; return 1; }
  local rc=0 f
  while IFS= read -r f; do
    [[ -z "$f" ]] && continue
    if [[ ${#DENIED_GLOBS[@]} -gt 0 ]] && matches_any "$f" DENIED_GLOBS; then echo "DENIED  $f"; rc=1; continue; fi
    if matches_any "$f" ALLOWED_GLOBS; then echo "ok      $f"; else echo "OUTSIDE $f"; rc=1; fi
  done < <(changed_files)
  return $rc
}
check_forbidden_patterns() {
  local rc=0 entry regex paths hits
  for entry in "${FORBIDDEN_PATTERNS[@]}"; do
    if [[ "$entry" == *" @@ "* ]]; then regex="${entry%% @@ *}"; paths="${entry#* @@ }"; else regex="$entry"; paths="."; fi
    # shellcheck disable=SC2086
    hits="$(grep -rIEn --exclude-dir=.git -e "$regex" -- $paths 2>/dev/null || true)"
    if [[ -n "$hits" ]]; then echo "FORBIDDEN /$regex/ found:"; echo "$hits"; rc=1; else echo "ok /$regex/ absent"; fi
  done
  return $rc
}
check_forbidden_paths() { local rc=0 p; for p in "${FORBIDDEN_PATHS[@]}"; do if [[ -e "$p" ]]; then echo "EXISTS $p"; rc=1; else echo "ok absent $p"; fi; done; return $rc; }
check_required_paths()  { local rc=0 p; for p in "${REQUIRED_PATHS[@]}";  do if [[ -e "$p" ]]; then echo "ok $p"; else echo "MISSING $p"; rc=1; fi; done; return $rc; }

# progress.md: checklist == task.md checklist modulo [ ]/[x]; all leaves checked;
# parent checked <=> all children checked; STATE=DONE; CURRENT=NONE.
checklist_lines() { awk '/<!-- checklist:start -->/{f=1;next} /<!-- checklist:end -->/{f=0} f' "$1"; }
check_progress() {
  [[ -f "$PROGRESS_FILE" ]] || { echo "missing $PROGRESS_FILE"; return 1; }
  [[ -f "$TASK_FILE" ]] || { echo "missing $TASK_FILE"; return 1; }
  local t p
  t="$(checklist_lines "$TASK_FILE"     | sed -E 's/^( *)- \[[ x]\] /\1- [ ] /')"
  p="$(checklist_lines "$PROGRESS_FILE" | sed -E 's/^( *)- \[[ x]\] /\1- [ ] /')"
  [[ -n "$t" ]] || { echo "no checklist block in $TASK_FILE"; return 1; }
  if [[ "$t" != "$p" ]]; then echo "checklist text/structure differs from task.md:"; diff <(echo "$t") <(echo "$p") || true; return 1; fi
  checklist_lines "$PROGRESS_FILE" | awk '
    function depth(s,  n){ n=match(s,/[^ ]/); return (n-1)/4 }
    {
      if ($0 !~ /^( {4}){0,3}- \[[ x]\] \*\*[0-9]+(\.[0-9]+){0,3}\*\* /) { print "BAD LINE: " $0; bad=1; next }
      d=depth($0); id=$0; match(id,/\*\*[0-9.]+\*\*/); id=substr(id,RSTART+2,RLENGTH-4)
      n=split(id,parts,"."); if (n!=d+1) { print "DEPTH/ID MISMATCH: " $0; bad=1 }
      cnt++; ids[cnt]=id; dep[cnt]=d; chk[cnt]=($0 ~ /- \[x\]/)
    }
    END {
      for (i=1;i<=cnt;i++){ leaf[i]=1; if (i<cnt && dep[i+1]>dep[i]) leaf[i]=0 }
      for (i=1;i<=cnt;i++){
        if (leaf[i]) { leaves++; if (chk[i]) done++; else { print "UNCHECKED LEAF " ids[i]; bad=1 } }
        else {
          allkids=1
          for (j=i+1;j<=cnt && dep[j]>dep[i];j++) if (!chk[j]) allkids=0
          if (chk[i] && !allkids) { print "PARENT CHECKED WITH UNCHECKED CHILD " ids[i]; bad=1 }
          if (!chk[i] && allkids) { print "PARENT UNCHECKED BUT CHILDREN DONE " ids[i]; bad=1 }
        }
      }
      printf "leaves=%d checked=%d\n", leaves, done
      exit bad
    }' || return 1
  local state cur
  state="$(sed -nE 's/^STATE: *([A-Z_]+).*/\1/p' "$PROGRESS_FILE" | head -1)"
  cur="$(sed -nE 's/^CURRENT: *([0-9.]+|NONE).*/\1/p' "$PROGRESS_FILE" | head -1)"
  [[ "$state" == "DONE" ]] || { echo "STATE=$state (want DONE)"; return 1; }
  [[ "$cur" == "NONE" ]]   || { echo "CURRENT=$cur (want NONE)"; return 1; }
  echo "ok STATE=DONE CURRENT=NONE"
}

# ---------- run ----------
run_check protected_manifest  check_protected_manifest
run_check scope               check_scope
run_check required_paths      check_required_paths
run_check forbidden_paths     check_forbidden_paths
run_check forbidden_patterns  check_forbidden_patterns
run_cmd_list focused    FOCUSED_CMDS
run_cmd_list regression REGRESSION_CMDS
run_cmd_list lint       LINT_CMDS
if [[ -n "$PROGRESS_FILE" ]]; then run_check progress check_progress; fi
for name in "${EXTRA_CHECKS[@]}"; do run_check "$name" "check_$name"; done
finish
