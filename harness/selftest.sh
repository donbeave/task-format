#!/usr/bin/env bash
# selftest.sh — prove the dispatch tools and the gate on a throwaway fixture. No network, no toolchain.
#
#   selftest.sh            (exit 0 = every scenario behaved as specified)
#
# Scenarios: lint accepts the example and rejects broken contracts; progress-init output is verify-shaped;
# verify.sh FAILS on the fresh progress file, FAILS on each tamper, PASSES only on the fully-checked DONE file.
set -Eeuo pipefail
T="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMPLATE="$T/../reference/task-template"
EX="$T/testdata/example"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/task-selftest.XXXXXX")"; trap 'rm -rf "$TMP"' EXIT
FAILS=0
expect() {  # expect NAME want-rc CMD...   (stdout/stderr captured; shown on mismatch)
  local name="$1" want="$2"; shift 2; local rc=0 out
  out="$("$@" 2>&1)" || rc=$?
  if [[ "$rc" == "$want" ]]; then printf 'ok   %-42s rc=%s\n' "$name" "$rc"
  else printf 'FAIL %-42s rc=%s want=%s\n' "$name" "$rc" "$want"; printf '%s\n' "$out" | sed 's/^/  | /'; FAILS=$((FAILS+1)); fi
}

# ---- repo rule: every AGENTS.md has a sibling CLAUDE.md symlink pointing at it ----
check_symlinks() { local f d rc=0; while IFS= read -r f; do d="$(dirname "$f")"
  [[ -L "$d/CLAUDE.md" && "$(readlink "$d/CLAUDE.md")" == "AGENTS.md" ]] || { echo "no CLAUDE.md -> AGENTS.md symlink next to $f"; rc=1; }
  done < <(find "$T/.." -name AGENTS.md -not -path '*/.git/*' -not -path '*/runs/*'); return $rc; }
expect "repo: AGENTS.md/CLAUDE.md symlinks" 0 check_symlinks

# ---- lint ----
expect "lint: example passes"         0 "$T/task-lint.sh" "$EX"
expect "lint: template has placeholders" 1 "$T/task-lint.sh" "$TEMPLATE"
mkdir -p "$TMP/bad"; cp "$EX/verify.config" "$TMP/bad/"
sed -E 's/^(    - \[ \] \*\*3\.2\*\*)/\1X/' "$EX/README.md" > "$TMP/bad/README.md"   # breaks the line grammar
expect "lint: broken checklist grammar"  1 "$T/task-lint.sh" "$TMP/bad"
awk '!/\*\*3\.1\*\*/' "$EX/README.md" > "$TMP/bad/README.md"                     # 3.2 without 3.1 => non-contiguous
expect "lint: non-contiguous IDs"        1 "$T/task-lint.sh" "$TMP/bad"
sed 's/^id: TASK-042/id: TASK-043/' "$EX/README.md" > "$TMP/bad/README.md"       # H1 no longer matches id
expect "lint: H1/id mismatch"            1 "$T/task-lint.sh" "$TMP/bad"

# ---- workspace: git repo tagged baseline, task dir with empty manifest ----
W="$TMP/work"; TD="$TMP/task"; mkdir -p "$W" "$TD"
( cd "$W" && git init -q && git -c user.name=t -c user.email=t@t commit -q --allow-empty -m baseline && git tag baseline )
cp "$EX/README.md" "$TEMPLATE/verify.sh" "$TD/"; : > "$TD/protected.sha256"
cat > "$TD/verify.config" <<'CFG'
BASE_REF="baseline"
FOCUSED_CMDS=(); REGRESSION_CMDS=(); LINT_CMDS=()
FORBIDDEN_PATTERNS=(); FORBIDDEN_PATHS=(); REQUIRED_PATHS=()
ALLOWED_GLOBS=("*"); DENIED_GLOBS=(); EXTRA_CHECKS=()
CFG
P="$TMP/progress.md"
"$T/progress-init.sh" "$EX" -o "$P" 2>/dev/null
gate() { VERIFY_ROOT="$W" VERIFY_TASK_DIR="$TD" PROGRESS_FILE="$1" VERIFY_LOG_DIR="$TMP/logs" "$TD/verify.sh"; }
gate_done() { local out; out="$(gate "$1")" || { printf '%s\n' "$out"; return 1; }; [[ "$(printf '%s\n' "$out" | tail -n1)" == "DONE" ]] || { printf '%s\n' "$out"; return 1; }; }

expect "progress-init: header shape"   0 grep -qxE 'TASK: TASK-042|STATE: IN_PROGRESS|CURRENT: 1\.1|BASELINE: <not run>' "$P"
expect "progress-init: checklist verbatim" 0 diff <(awk '/checklist:start/{f=1;next} /checklist:end/{f=0} f' "$EX/README.md") <(awk '/checklist:start/{f=1;next} /checklist:end/{f=0} f' "$P")
expect "gate: fresh progress fails"    1 gate "$P"

# fully done copy
D="$TMP/done.md"
sed -e 's/^- \[ \]/- [x]/; s/^\(    *\)- \[ \]/\1- [x]/' -e 's/^STATE: .*/STATE: DONE/; s/^CURRENT: .*/CURRENT: NONE/; s/^BASELINE: .*/BASELINE: cargo test -p auth expired_refresh_token -> 1 failed/' "$P" > "$D"
expect "gate: done progress passes DONE" 0 gate_done "$D"

# tampers, each must fail
sed 's/^STATE: DONE/STATE: IN_PROGRESS/' "$D" > "$TMP/t1.md";                       expect "gate: STATE not DONE"          1 gate "$TMP/t1.md"
sed 's/^CURRENT: NONE/CURRENT: 4.2/' "$D" > "$TMP/t2.md";                           expect "gate: CURRENT not NONE"        1 gate "$TMP/t2.md"
sed 's/^BASELINE: .*/BASELINE: <not run>/' "$D" > "$TMP/t3.md";                     expect "gate: BASELINE not recorded"   1 gate "$TMP/t3.md"
sed 's/^TASK: TASK-042/TASK: TASK-041/' "$D" > "$TMP/t4.md";                        expect "gate: TASK id mismatch"        1 gate "$TMP/t4.md"
sed -E 's/^(    - \[)x(\] \*\*3\.1\*\*)/\1 \2/' "$D" > "$TMP/t5.md";                    expect "gate: parent checked, child not" 1 gate "$TMP/t5.md"
sed -E 's/^(- \[)x(\] \*\*3\*\*)/\1 \2/' "$D" > "$TMP/t6.md";                       expect "gate: parent unchecked, kids done" 1 gate "$TMP/t6.md"
sed -E 's/(\*\*3\.1\*\*) /\1 (reworded) /' "$D" > "$TMP/t7.md";                     expect "gate: reworded checklist text" 1 gate "$TMP/t7.md"
awk '!/\*\*3\.2\*\*/' "$D" > "$TMP/t8.md";                                          expect "gate: deleted checklist line"  1 gate "$TMP/t8.md"
printf '    - [x] **4.3** extra — evidence: none.\n' > "$TMP/extra"; awk -v e="$TMP/extra" '/checklist:end/{while((getline l<e)>0)print l} {print}' "$D" > "$TMP/t9.md"
                                                                                     expect "gate: added checklist line"    1 gate "$TMP/t9.md"
expect "gate: missing progress file"   1 gate "$TMP/nope.md"

printf 'SELFTEST %s\n' "$([[ $FAILS -eq 0 ]] && echo PASS || echo "FAIL ($FAILS)")"
[[ $FAILS -eq 0 ]]
