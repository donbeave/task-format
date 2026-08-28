#!/usr/bin/env bash
# task-lint.sh — author checklist for one task package (schema task/v3).
#
#   task-lint.sh <task-dir | README.md>
#
# Exit 0 = every rule passed. Exit 1 = at least one ERROR. Warnings never fail.
# Output: one line per finding, "ERROR <rule>: <detail>" or "WARN <rule>: <detail>", then "LINT PASS|FAIL".
#
# Checked (see PACKAGE.md "Author checklist"):
#   frontmatter   schema task/v3; id TASK-<n>; kind in the allowed set; verify; expected_paths non-empty
#   sections      every required H2 present, in template order
#   placeholders  no template placeholders left (TASK-000, P-NNN, <command>, <expected>, ...)
#   preconditions every P-NNN line carries a backticked command
#   acceptance    every AC-NNN row has an evidence command and an expected result
#   checklist     one block; line grammar; IDs contiguous; depth = ID components; max depth 4;
#                 5-20 leaves; every leaf has "evidence:"; no single-child parent; every AC-* referenced;
#                 last leaf is the verify.sh gate
#   verify.config (if present next to README.md) BASE_REF set; ALLOWED_GLOBS == expected_paths;
#                 every protected_path matches a DENIED_GLOB; at least one FOCUSED_CMD; no template placeholders
#   size          README.md over 10,000 bytes (~2,500 tokens) is a warning
set -Eeuo pipefail

target="${1:-.}"
if [[ -d "$target" ]]; then TASK_DIR="$target"; else TASK_DIR="$(dirname "$target")"; fi
TASK_DIR="$(cd "$TASK_DIR" && pwd)"
README="$TASK_DIR/README.md"
CONFIG="$TASK_DIR/verify.config"
[[ -f "$README" ]] || { echo "ERROR readme: missing $README"; echo "LINT FAIL"; exit 1; }

ERRORS=0; WARNS=0
err()  { printf 'ERROR %s: %s\n' "$1" "$2"; ERRORS=$((ERRORS+1)); }
warn() { printf 'WARN  %s: %s\n' "$1" "$2"; WARNS=$((WARNS+1)); }

# ---------- frontmatter ----------
fm="$(awk 'NR==1 && $0!="---"{exit} NR==1{next} /^---$/{exit} {print}' "$README")"
[[ -n "$fm" ]] || err frontmatter "no YAML frontmatter block at top of README.md"
fm_val() { printf '%s\n' "$fm" | sed -nE "s/^$1: *\"?([^\"#]*[^\" #])\"? *(#.*)?$/\1/p" | head -1; }
fm_list() {  # fm_list KEY -> one item per line (block list "  - \"x\"")
  printf '%s\n' "$fm" | awk -v k="$1" '
    $0 ~ "^"k":" { f=1; next }
    f && /^[^ ]/ { f=0 }
    f && /^ *- / { s=$0; sub(/^ *- */,"",s); sub(/ *#.*$/,"",s); gsub(/^"|"$/,"",s); if (s!="") print s }'
}
schema="$(fm_val schema)"; id="$(fm_val id)"; kind="$(fm_val kind)"; verify="$(fm_val verify)"; title="$(fm_val title)"
[[ "$schema" == "task/v3" ]] || err frontmatter "schema is '${schema:-<missing>}', want task/v3"
[[ "$id" =~ ^TASK-[0-9]+$ ]] || err frontmatter "id is '${id:-<missing>}', want TASK-<digits>"
[[ -n "$title" ]] || err frontmatter "title missing"
case "$kind" in bugfix|feature|refactor|removal|migration|test|docs) ;; *) err frontmatter "kind is '${kind:-<missing>}'";; esac
[[ -n "$verify" ]] || err frontmatter "verify missing"
mapfile -t EXPECTED  < <(fm_list expected_paths)
mapfile -t PROTECTED < <(fm_list protected_paths)
[[ ${#EXPECTED[@]} -gt 0 ]] || err frontmatter "expected_paths empty"

# ---------- sections ----------
want_sections=("Goal" "Context" "Preconditions" "Scope" "Requirements" "Acceptance criteria" "Fixed decisions" "Checklist")
mapfile -t have_sections < <(sed -nE 's/^## +(.*[^ ]) *$/\1/p' "$README")
missing=(); for s in "${want_sections[@]}"; do found=0; for h in "${have_sections[@]}"; do [[ "$h" == "$s" ]] && found=1; done; [[ $found -eq 1 ]] || missing+=("$s"); done
[[ ${#missing[@]} -eq 0 ]] || err sections "missing H2: ${missing[*]}"
if [[ ${#missing[@]} -eq 0 ]]; then
  order="$(printf '%s\n' "${have_sections[@]}" | grep -nxF -f <(printf '%s\n' "${want_sections[@]}") | cut -d: -f1 | tr '\n' ' ')"
  sorted="$(printf '%s\n' $order | sort -n | tr '\n' ' ')"
  [[ "$order" == "$sorted" ]] || err sections "H2 order differs from template order"
fi
[[ "$(sed -nE 's/^# +(TASK-[0-9]+).*/\1/p' "$README" | head -1)" == "$id" ]] || err heading "H1 must start with '# $id — '"

# ---------- placeholders ----------
ph="$(grep -nE 'TASK-000|\b[PRD]-NNN\b|AC-NNN|<command>|<expected>|<result>|<state>|<imperative|<One sentence|<area>' "$README" || true)"
[[ -z "$ph" ]] || err placeholders "template placeholders left:"$'\n'"$(printf '%s\n' "$ph" | sed 's/^/    /')"

# ---------- preconditions ----------
while IFS= read -r line; do
  pid="$(printf '%s' "$line" | sed -nE 's/^- \*\*(P-[0-9]+):\*\*.*/\1/p')"
  [[ -n "$pid" ]] || continue
  printf '%s' "$line" | grep -qE -- '— *`[^`]+`' || err preconditions "$pid has no backticked command"
done < <(awk '/^## Preconditions/{f=1;next} /^## /{f=0} f' "$README")
grep -qE '^- \*\*P-[0-9]+:\*\*' "$README" || err preconditions "no P-NNN entries"

# ---------- acceptance table ----------
mapfile -t AC_IDS < <(sed -nE 's/^\| *(AC-[0-9]+) *\|.*/\1/p' "$README")
[[ ${#AC_IDS[@]} -gt 0 ]] || err acceptance "no AC-NNN rows"
while IFS= read -r row; do
  acid="$(printf '%s' "$row" | sed -nE 's/^\| *(AC-[0-9]+) *\|.*/\1/p')"; [[ -n "$acid" ]] || continue
  IFS='|' read -r _ _ gwt cmd expected _ <<<"$row"
  [[ "$cmd" =~ \`[^\`]+\` ]] || err acceptance "$acid evidence column has no backticked command"
  [[ -n "${expected// /}" ]] || err acceptance "$acid expected column empty"
  [[ -n "${gwt// /}" ]] || err acceptance "$acid Given/When/Then empty"
done < <(awk '/^## Acceptance criteria/{f=1;next} /^## /{f=0} f' "$README")

# ---------- checklist ----------
nblocks="$(grep -c '<!-- checklist:start -->' "$README" || true)"
[[ "$nblocks" -eq 1 ]] || err checklist "expected exactly one <!-- checklist:start --> marker, found $nblocks"
[[ "$(grep -c '<!-- checklist:end -->' "$README" || true)" -eq 1 ]] || err checklist "expected exactly one <!-- checklist:end --> marker"
cl="$(awk '/<!-- checklist:start -->/{f=1;next} /<!-- checklist:end -->/{f=0} f' "$README")"
[[ -n "$cl" ]] || err checklist "checklist block empty"
if [[ -n "$cl" ]]; then
  cl_out="$(printf '%s\n' "$cl" | awk -v acs="${AC_IDS[*]}" '
    function depth(s,  n){ n=match(s,/[^ ]/); return (n-1)/4 }
    {
      if ($0 !~ /^( {4}){0,3}- \[ \] \*\*[0-9]+(\.[0-9]+){0,3}\*\* /) { print "ERROR checklist: bad line: " $0; bad=1; next }
      d=depth($0); id=$0; match(id,/\*\*[0-9.]+\*\*/); id=substr(id,RSTART+2,RLENGTH-4)
      n=split(id,parts,"."); if (n!=d+1) { print "ERROR checklist: depth/ID mismatch: " id; bad=1 }
      if (d>3) { print "ERROR checklist: depth > 4: " id; bad=1 }
      cnt++; ids[cnt]=id; dep[cnt]=d; text[cnt]=$0
      if (id in seen) { print "ERROR checklist: duplicate ID " id; bad=1 }; seen[id]=1
      # contiguity: expected next sibling / first child
      if (cnt==1) { if (id!="1") { print "ERROR checklist: first ID must be 1, got " id; bad=1 } }
      else {
        pd=dep[cnt-1]; pid=ids[cnt-1]
        if (d==pd+1) want=pid ".1"
        else if (d<=pd) { split(pid,pp,"."); want=""; for (i=1;i<=d+1;i++) { v=pp[i]; if (i==d+1) v=v+1; want=want (i>1?".":"") v } }
        else { print "ERROR checklist: depth jumps by more than one at " id; bad=1; want=id }
        if (id!=want) { print "ERROR checklist: expected ID " want " after " pid ", got " id; bad=1 }
      }
    }
    END {
      for (i=1;i<=cnt;i++){ leaf[i]=1; if (i<cnt && dep[i+1]>dep[i]) leaf[i]=0 }
      for (i=1;i<=cnt;i++){
        if (leaf[i]) { leaves++; last=ids[i]; if (text[i] !~ /evidence:/) { print "ERROR checklist: leaf " ids[i] " has no \"evidence:\""; bad=1 } }
        else { kids=0; for (j=i+1;j<=cnt && dep[j]>dep[i];j++) if (dep[j]==dep[i]+1) kids++; if (kids==1) { print "ERROR checklist: parent " ids[i] " has a single child"; bad=1 } }
      }
      if (leaves<5 || leaves>20) { print "ERROR checklist: " leaves " leaves, want 5-20"; bad=1 }
      all=""; for (i=1;i<=cnt;i++) all=all "\n" text[i]
      m=split(acs,a," "); for (i=1;i<=m;i++) if (index(all,"`" a[i] "`")==0) { print "ERROR checklist: " a[i] " not referenced by any checklist item"; bad=1 }
      if (last!="" && text[cnt] !~ /verify\.sh/) { print "ERROR checklist: last leaf " last " must be the verify.sh gate leaf"; bad=1 }
      printf "INFO checklist: items=%d leaves=%d\n", cnt, leaves
      exit bad
    }')" && cl_rc=0 || cl_rc=$?
  printf '%s\n' "$cl_out" | grep -v '^INFO' || true
  [[ $cl_rc -eq 0 ]] || ERRORS=$((ERRORS + $(printf '%s\n' "$cl_out" | grep -c '^ERROR')))
fi

# ---------- verify.config ----------
if [[ -f "$CONFIG" ]]; then
  cfg_dump="$(bash -c '
    set -e; BASE_REF=""; FOCUSED_CMDS=(); ALLOWED_GLOBS=(); DENIED_GLOBS=()
    source "$1" >/dev/null 2>&1
    printf "BASE_REF=%s\n" "$BASE_REF"
    printf "FOCUSED=%s\n" "${#FOCUSED_CMDS[@]}"
    for g in "${ALLOWED_GLOBS[@]}"; do printf "A %s\n" "$g"; done
    for g in "${DENIED_GLOBS[@]}";  do printf "D %s\n" "$g"; done' _ "$CONFIG" 2>&1)" || { err config "verify.config does not source cleanly:"$'\n'"$cfg_dump"; cfg_dump=""; }
  if [[ -n "$cfg_dump" ]]; then
    base="$(printf '%s\n' "$cfg_dump" | sed -n 's/^BASE_REF=//p')"; [[ -n "$base" ]] || err config "BASE_REF empty"
    nfoc="$(printf '%s\n' "$cfg_dump" | sed -n 's/^FOCUSED=//p')"; [[ "${nfoc:-0}" -gt 0 ]] || err config "FOCUSED_CMDS empty"
    mapfile -t ALLOWED < <(printf '%s\n' "$cfg_dump" | sed -n 's/^A //p')
    mapfile -t DENIED  < <(printf '%s\n' "$cfg_dump" | sed -n 's/^D //p')
    a_sorted="$(printf '%s\n' "${ALLOWED[@]}" | sort -u)"; e_sorted="$(printf '%s\n' "${EXPECTED[@]}" | sort -u)"
    [[ "$a_sorted" == "$e_sorted" ]] || err config "ALLOWED_GLOBS != expected_paths:"$'\n'"$(diff <(echo "$e_sorted") <(echo "$a_sorted") | sed 's/^/    /' || true)"
    # shellcheck disable=SC2053  # glob match against DENIED_GLOBS is intended
    for p in "${PROTECTED[@]}"; do
      hit=0; for d in "${DENIED[@]}"; do [[ "$p" == $d ]] && hit=1; done
      [[ $hit -eq 1 ]] || err config "protected path '$p' matches no DENIED_GLOBS entry"
    done
    cfg_code="$(grep -vE '^[[:space:]]*#' "$CONFIG")"
    grep -qE '<[a-z_ -]+>' <<<"$cfg_code" && err config "template placeholders left in verify.config: $(grep -oE '<[a-z_ -]+>' <<<"$cfg_code" | sort -u | tr '\n' ' ')"
    grep -q '"ERE|' "$CONFIG" && warn config "stale comment: FORBIDDEN_PATTERNS separator is ' @@ ', not '|'"
  fi
else
  warn config "no verify.config next to README.md"
fi

# ---------- size ----------
bytes="$(wc -c <"$README" | tr -d ' ')"
[[ "$bytes" -le 10000 ]] || warn size "README.md is $bytes bytes (~$((bytes/4)) tokens); target under ~2,500 tokens"

printf 'SUMMARY errors=%s warnings=%s\n' "$ERRORS" "$WARNS"
if [[ $ERRORS -eq 0 ]]; then echo "LINT PASS"; exit 0; fi
echo "LINT FAIL"; exit 1
