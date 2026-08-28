# Progress state and verifier design (Q1 / Q2)

Research note. Inputs: `docs/research/raw/01-initial-goal-template.md`, `docs/research/raw/02-checklist-checkboxes.md`, external sources listed per section. Scripts below were executed against a throwaway fixture repo on macOS (bash 5.3, git 2.50) — positive path prints `DONE`, and each negative scenario (protected-file tamper, out-of-scope file, forbidden pattern, inconsistent checklist, tampered `verify.sh` + regenerated manifest) fails with a structured line. Not yet run inside the Docker harness (UNVERIFIED there).

---

## Q1. Where does progress state live?

### Evidence

| Source | What it says | Relevance |
| --- | --- | --- |
| Anthropic, *Effective harnesses for long-running agents* | `claude-progress.txt` log + `feature_list.json`; agents only flip `passes`; "we landed on using JSON for this, as the model is less likely to inappropriately change or overwrite JSON files compared to Markdown files"; "It is unacceptable to remove or edit tests"; fresh session = `pwd`, read git log + progress file, pick next feature. | Spec/state separation; agents DO rewrite markdown they are allowed to edit; git log is part of resume protocol. |
| Anthropic, *Harness design for long-running apps* | File-based handoffs between planner/generator/evaluator; "tuning a standalone evaluator to be skeptical turns out to be far more tractable than making a generator critical of its own work". | Evaluator must not depend on generator-owned state for the verdict. |
| OpenAI, *Codex exec plans* (`.agent/PLANS.md`) | Plan is "a living document"; Progress section with `- [x] (2025-10-01 13:00Z) step`; "must always reflect the actual current state"; "restart from _only_ the ExecPlan and no other work". | Checkbox + timestamp convention; single-file restart. But the plan there is agent-authored and mutable by design — the opposite trust model from an immutable contract. |
| OpenAI, *Follow goals* (Codex) | Status update should "name the current checkpoint, what was verified, what remains, and whether Codex is blocked"; keep a "concise progress log". | Per-turn projection line (`GOAL_PROGRESS`) is aligned. |
| Claude Code `/goal` docs | Evaluator "doesn't run commands or read files independently"; wraps a prompt-based Stop hook; goal survives `--resume`; context overflow that auto-compaction can't clear kills the goal. | Progress file is for the *agent's* resume, not the evaluator; the evaluator only sees transcript. |
| Claude Code hooks docs + issue #15174 | `SessionStart` matcher `compact` exists and stdout is added to context; issue #15174 (v2.0.76, Dec 2025) reported compact-matcher stdout NOT injected, closed as duplicate; `PostCompact` event now exists (UNVERIFIED whether it injects context). Reporter's workaround: put reminder in `CLAUDE.md`, which is reloaded after compaction. | Post-compaction re-read of progress must not rely solely on a hook; put "on start/resume read progress.md" into CLAUDE.md/AGENTS.md of the fixture repo as well. |
| Ralph Wiggum (ghuntley `how-to-ralph-wiggum`; Anthropic `ralph-wiggum` plugin) | `IMPLEMENTATION_PLAN.md` "kept up to date with items considered complete/incomplete", updated in place then `git commit`; each iteration = fresh context = one item = one commit; plugin: "The prompt never changes between iterations. Claude's previous work persists in files… and git history". Plugin: completion promise is exact-string match, "Always rely on --max-iterations". | Plan-as-mutable-state works when the plan is *owned* by the agent. Git history is a second ledger, not primary. |
| Beads (`bd`) | "Do not use markdown TODO lists for task tracking"; Dolt/SQLite as truth, JSONL as export; atomic `bd update --claim` / `bd close`; argument: markdown "not queryable", high parse load. | Structured state wins for queries and multi-agent; overkill for a single one-task run with ≤25 leaves, and adds a binary dependency to every fixture container. |
| GitHub task lists | `- [ ]`/`- [x]` binary only; nested via indentation; "Tasklist blocks are retired" (sub-issues replace them). | No standard third state; our "current leaf" pointer approach stands. |
| aider, *Code in JSON* | Models score worse when forced to emit code via JSON vs markdown. | Counterweight to Anthropic's JSON preference: JSON is safer against *accidental* edits, worse as an *editing target*. Net: markdown file the agent edits, machine-checked grammar. |
| SWE-bench harness | Model patch applied, then the *gold test patch* is applied over it in a fresh container; grading from parsed logs, `FAIL_TO_PASS` + `PASS_TO_PASS`. | The trusted verifier re-imposes protected test files rather than trusting the workspace copy. Same idea as our external manifest/trusted copy. |

### Options

| Criterion | A. Checkboxes in `task.md` (marked mutable regions) — raw/02 design | B. `task.md` read-only + `progress.md` carrying a copy of the checklist (**recommended**) | C. `progress.json`/`.yaml` machine state | D. Git commits as the only ledger |
| --- | --- | --- | --- | --- |
| Fresh-context resume after compaction | Good: one file, everything visible. | Good: two files, both named in the invocation and in the header of `progress.md`; `progress.md` is self-contained (header + checklist + log). | Medium: agent must mentally join JSON ids to checklist text in `task.md`. | Poor: needs `git log` archaeology; no "current leaf" pointer; agents may not commit. |
| Deterministic outer-gate diffing | Needs region-aware normalizer; any prose change outside regions must be detected; parser is the trust boundary. | Trivial: whole-file sha256 of `task.md`; `progress.md` checklist normalized (`[x]`→`[ ]`) and compared byte-for-byte to `task.md` checklist block. Tested (`check_progress`). | Trivial: schema validate; but the agent can silently drop entries — need id-set equality check anyway. | Weak: commit messages unstructured. |
| Read-only mount feasibility | **Impossible** — file must be writable. | Yes: mount `task.md`, `verify.sh`, `verify.config`, `protected.sha256` read-only; only `progress.md` writable. | Yes. | Yes. |
| Agent compliance likelihood | Risky: Anthropic observed models "inappropriately change or overwrite" markdown they can edit; the contract and the state sit in one buffer. Region markers are prose-level protection. | Good: edits confined to a file that is *supposed* to change; strict line grammar checked by gate; identical checklist text lowers "helpful rewording" incentive because diff vs `task.md` fails the gate. | Best against accidental edits (Anthropic), worse as edit target (aider); agents tend to regenerate whole JSON files. | Medium; Ralph relies on it but with a mutable plan file too. |
| Human readability | Good. | Good: `progress.md` reads as a status board; `task.md` reads as a contract. | Poor without tooling. | Poor. |
| Divergence risk (checklist copies) | None (single copy). | Exists, **but converted into a gate failure**: normalized copy must equal `task.md` block. Copy is generated at dispatch by tooling, not typed by the agent. | Same as B (ids must match). | n/a |

Rejected: A because it kills read-only mounts and whole-file hashing, which raw/01 lists as the primary protection mechanism ("Verifier and trusted inputs are protected outside the executor's control, not merely protected by prose"). C because the agent's main interaction with progress is *editing* it, and the human operator wants to eyeball it; the JSON advantage (harder to corrupt) is reproduced by gate-checking the markdown grammar. D as sole ledger because commits are optional, squashable, and carry no current-leaf pointer.

### Recommendation

**B, with commits as a secondary ledger.**

1. `task.md` is byte-immutable. It contains the canonical numbered checklist (text ties leaves to `R-*`/`AC-*`). Whole-file sha256 in the manifest; read-only mount where the harness permits.
2. `progress.md` is the only agent-writable task file. Generated at dispatch from `task.md` (checklist block copied verbatim, all `[ ]`). Contains: fixed-key header, checklist copy, checkpoint log, handoff fields.
3. Gate rules for `progress.md` (all enforced by `check_progress` in `verify.sh`; full run only at `DONE`, a `--partial` mode for mid-run linting is a follow-up):
   - Checklist block normalized (`[x]`→`[ ]`) must equal `task.md` block byte-for-byte (no reword, reorder, re-indent, add, delete).
   - Line grammar: `^( {4}){0,3}- \[[ x]\] \*\*N(\.N){0,3}\*\* ` ; depth = id components − 1.
   - Parent checked ⇔ all descendants checked.
   - Exactly one unchecked leaf named in `CURRENT:` while `STATE ∈ {IN_PROGRESS, VERIFYING, BLOCKED, NEEDS_REPLAN}`; `NONE` only in `NOT_STARTED`/`DONE`.
   - `LEAVES: done/total` must equal computed leaf counts.
   - At `DONE`: every leaf checked, `STATE: DONE`, `CURRENT: NONE`.
4. Agent commits after every checked leaf: `TASK-042 2.1.1.1: <what>` — git log becomes a timestamped mirror, and `git log --oneline` is part of the resume protocol (Anthropic). Not authoritative.
5. Resume protocol (put in `task.md` "Required execution sequence" AND in the fixture repo's `CLAUDE.md`/`AGENTS.md`, because the `SessionStart compact` hook path is UNVERIFIED for context injection): read `task.md`, read `progress.md`, `git status`, `git diff --stat`, `git log --oneline -20`, then continue from `CURRENT`.
6. Per-turn transcript projection line (`GOAL_PROGRESS …`) kept from raw/02 — it is what the `/goal` evaluator can actually see.

### `progress.md` format (v1)

Header = fixed `KEY: value` lines (grep-able, one per line, order fixed). Checklist between HTML-comment markers, copied verbatim from `task.md`. Log = append-only, one entry per line, `|`-separated. Timestamps ISO-8601 UTC.

```markdown
# TASK-042 progress

TASK: TASK-042
TASK_FILE: .task/task.md
STATE: IN_PROGRESS
CURRENT: 2.1.1.1
LEAVES: 3/9
BASELINE: cargo test expired_refresh_token — FAILED as expected (exit 101, 1 failed)
UPDATED: 2026-08-28T10:15:00Z

<!-- checklist:start -->
- [x] **1** Establish a verified starting point.
    - [x] **1.1** Read all required context — complete when constraints and code flow are stated in the transcript.
    - [x] **1.2** Confirm preconditions `P-001..P-003` — complete when each is evidenced by a command or artifact.
    - [x] **1.3** Run the documented baseline — complete when the pre-change result is recorded in BASELINE.
- [ ] **2** Implement the bounded required behavior.
    - [ ] **2.1** Satisfy `R-001`, `R-002` — complete when descendant checks pass.
        - [ ] **2.1.1** Validate expiry before rotation (`R-001`) — complete when `cargo test expiry_precedes_rotation` exits 0.
            - [ ] **2.1.1.1** Move the expiry check ahead of the transaction (`R-001`, `AC-001`) — complete when `cargo test expired_refresh_token` exits 0.
        - [ ] **2.1.2** Return `refresh_token_expired` (`R-002`) — complete when `cargo test expired_error_code` exits 0.
    - [ ] **2.2** Remove `legacy_expiry_check` (`R-003`, `R-005`) — complete when `rg legacy_expiry_check src tests` is empty and regressions pass.
- [ ] **3** Prove every acceptance criterion.
    - [ ] **3.1** Prove `AC-001` — complete when `cargo test expired_refresh_token` exits 0.
    - [ ] **3.2** Prove `AC-002` — complete when `cargo test valid_refresh_rotation` exits 0.
- [ ] **4** Final review and trusted gate.
    - [ ] **4.1** Review diff against scope and protected paths — complete when `./verify.sh` scope/manifest checks PASS.
    - [ ] **4.2** Run `./verify.sh` — complete when exit 0 and last line `DONE`.
<!-- checklist:end -->

## Log

- 2026-08-28T09:40:00Z | 1.1 | COMPLETED | constraints stated in transcript; flow: handler -> SessionService::rotate -> TokenStore
- 2026-08-28T09:42:00Z | 1.2 | COMPLETED | P-001 git log shows TASK-041 merge; P-002 fixtures present; P-003 D-041 status=final
- 2026-08-28T09:45:00Z | 1.3 | COMPLETED | cargo test expired_refresh_token -> exit 101, 1 failed (expected)
- 2026-08-28T10:15:00Z | 2.1.1.1 | IN_PROGRESS | done: check moved before tx; remaining: counter still incremented on early return
- 2026-08-28T10:15:00Z | 2.1.1.1 | FAILED | cargo test expired_refresh_token: rotation_counter == 1, want 0; next: guard increment

## Handoff

CURRENT_FAILURE: expired_refresh_token asserts rotation_counter == 0, got 1
LAST_ACTION: moved expiry check before begin_transaction in session/rotate.rs
NEXT_ACTION: 2.1.1.1 — move counter increment inside the post-validation branch
DECISIONS: none
```

Rules (also stated in `task.md`):

- Legal log statuses: `COMPLETED | IN_PROGRESS | FAILED | REOPENED | BLOCKED`. A `[x]` without a `COMPLETED` entry for that id is invalid.
- Numbering: 1–4 levels, `N`, `N.N`, `N.N.N`, `N.N.N.N`; four spaces per level; siblings contiguous from 1; ids never reused.
- Leaf = item with no children. Only leaves count in `LEAVES`. Only a leaf may be `CURRENT`.
- Order: numeric depth-first unless `task.md` states dependencies otherwise.
- The checklist block is never edited except the three-character token. Missing work → `NEEDS_REPLAN`, not a new line.
- Log lines are append-only; corrections are new `REOPENED`/`FAILED` lines.
- `UPDATED` and `LEAVES` refreshed on every write; before any expected compaction or stop.

Why a checklist copy instead of id-only state: the resume reader (fresh agent) and the human both want the text next to the box; the copy costs nothing because it is generated and gate-diffed. Why HTML-comment markers rather than a heading: markers survive heading renames and are trivially `awk`-able.

---

## Q2. `verify.sh` design

### Trust model: two runs of the same script

| | Executor run (feedback loop) | Outer-gate run (trust) |
| --- | --- | --- |
| Invocation | `./verify.sh` from repo root (copy in workspace; read-only mount if possible) | `VERIFY_ROOT=/work VERIFY_CONFIG=/trusted/verify.config VERIFY_MANIFEST=/trusted/protected.sha256 /trusted/verify.sh` from outside the workspace |
| Script/config/manifest source | workspace copies (may be tampered) | copies stored outside the workspace at dispatch |
| What a tamper buys the agent | nothing: outer run ignores workspace `verify.sh`, `verify.config`, `protected.sha256`; workspace `verify.sh` is itself in the manifest, so the inner run also fails once it is edited (tested: run 6) | — |
| `VERIFY_SKIP` | may be used by a human developer while authoring fixtures; agent instructions forbid it; any skip → exit 3, no `DONE` | must be unset; gate should `env -i` |
| Additional gate-only checks | — | whole-file sha256 of `task.md` (in manifest); `git diff` scope; optionally re-clone into a fresh container and copy protected files *over* the workspace before running (SWE-bench "gold test patch" pattern) |

The script is identical in both places; behaviour differs only through env vars pointing at trusted inputs. That keeps the agent's feedback loop honest (same checks, same output) and removes any "works locally, fails in the gate" ambiguity except deliberate tampering.

### Output contract

- One line per check: `CHECK <name> PASS` | `CHECK <name> FAIL rc=<n> log=<path>` (+ indented 40-line tail) | `CHECK <name> SKIP (...)`.
- `SUMMARY pass=N fail=N skip=N log_dir=...`
- `RESULT PASS` then `DONE` as the very last line, exit 0 — only when fail=0 and skip=0.
- `RESULT PASS-WITH-SKIPS (not DONE)` exit 3; `RESULT FAIL` exit 1; missing config exit 2; internal error (trap ERR) exit 70 with `RESULT FAIL`.
- All checks run (no short-circuit) so the agent gets the full list; `VERIFY_FAIL_FAST=1` to stop early.

### `verify.config` (bash, sourced; per fixture repo)

```bash
# .task/verify.config — project-specific inputs for verify.sh (sourced by bash; keep it data-only)
BASE_REF="main"                                  # commit/branch the task started from; scope diff base
FOCUSED_CMDS=("cargo test expired_refresh_token" "cargo test valid_refresh_rotation")
REGRESSION_CMDS=("cargo test -p auth")
LINT_CMDS=("cargo fmt --check" "cargo clippy -p auth -- -D warnings")
FORBIDDEN_PATTERNS=('legacy_expiry_check|src tests' 'TODO\(TASK-042\)' '#\[ignore\]|tests')   # "ERE|paths…"; paths optional
FORBIDDEN_PATHS=("src/auth/legacy_expiry.rs")
REQUIRED_PATHS=("tests/expired_refresh_token.rs")
ALLOWED_GLOBS=("src/auth/*" "tests/*" ".task/progress.md" "Cargo.lock")
DENIED_GLOBS=(".task/task.md" ".task/verify.config" ".task/protected.sha256" "verify.sh" "tests/fixtures/*")
TASK_FILE=".task/task.md"
PROGRESS_FILE=".task/progress.md"                # empty string disables the progress check
EXTRA_CHECKS=()                                  # names; define check_<name>() functions here if needed
```

Glob semantics: bash `[[ $f == $glob ]]` — `*` matches across `/`, so `src/auth/*` covers nested paths; `?` and `[...]` work; no `**`. `DENIED_GLOBS` are evaluated first. Changed set = `git diff --name-only $BASE_REF` ∪ staged ∪ untracked-not-ignored.

### `verify.sh`

```bash
#!/usr/bin/env bash
# verify.sh — generic trusted completion gate for one task package.
# Contract: exit 0 AND last stdout line exactly "DONE" <=> every check passed.
# Runs all checks (no short-circuit) so the executor gets complete feedback;
# set VERIFY_FAIL_FAST=1 to stop at the first failure.
#
# Usage (executor, inside workspace):   ./verify.sh
# Usage (outer gate, trusted copy):     VERIFY_ROOT=/work VERIFY_CONFIG=/trusted/verify.config \
#                                       VERIFY_MANIFEST=/trusted/protected.sha256 /trusted/verify.sh
#
# Env overrides (all optional):
#   VERIFY_ROOT      repo root (default: git toplevel of cwd, else cwd)
#   VERIFY_CONFIG    path to verify.config (default: $VERIFY_ROOT/.task/verify.config)
#   VERIFY_MANIFEST  path to protected.sha256 (default: $VERIFY_ROOT/.task/protected.sha256)
#   VERIFY_LOG_DIR   where per-check logs go (default: mktemp -d)
#   VERIFY_FAIL_FAST 1 = stop at first failing check
#   VERIFY_SKIP      space-separated check names to skip (dev only; outer gate MUST leave unset)
set -Eeuo pipefail

# ---------- bootstrap ----------
VERIFY_ROOT="${VERIFY_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
cd "$VERIFY_ROOT"
VERIFY_CONFIG="${VERIFY_CONFIG:-$VERIFY_ROOT/.task/verify.config}"
VERIFY_MANIFEST="${VERIFY_MANIFEST:-$VERIFY_ROOT/.task/protected.sha256}"
VERIFY_LOG_DIR="${VERIFY_LOG_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/verify.XXXXXX")}"
VERIFY_FAIL_FAST="${VERIFY_FAIL_FAST:-0}"
VERIFY_SKIP="${VERIFY_SKIP:-}"
mkdir -p "$VERIFY_LOG_DIR"

# Any uncaught error => explicit FAIL, never a silent DONE.
trap 'rc=$?; printf "RESULT FAIL internal-error line=%s rc=%s\n" "$LINENO" "$rc" >&2; printf "RESULT FAIL\n"; exit 70' ERR

# ---------- config defaults (overridden by verify.config) ----------
BASE_REF="${BASE_REF:-}"                 # e.g. main or a commit sha; required for scope check
FOCUSED_CMDS=()                          # focused behaviour tests, e.g. "cargo test expired_refresh_token"
REGRESSION_CMDS=()                       # relevant regression suites
LINT_CMDS=()                             # fmt/lint/typecheck
FORBIDDEN_PATTERNS=()                    # "regex|path1 path2 ..." ; paths optional (default: tracked files)
FORBIDDEN_PATHS=()                       # files/dirs that must NOT exist
REQUIRED_PATHS=()                        # files/dirs that MUST exist
ALLOWED_GLOBS=()                         # changed files must match one of these (bash [[ == ]] globs)
DENIED_GLOBS=()                          # changed files must match none of these (checked first)
PROGRESS_FILE=""                         # optional: .task/progress.md ; enables progress consistency check
TASK_FILE=""                             # optional: .task/task.md ; needed for progress check
EXTRA_CHECKS=()                          # names of user-defined bash functions "check_<name>" in config

if [[ -f "$VERIFY_CONFIG" ]]; then
  # shellcheck disable=SC1090
  source "$VERIFY_CONFIG"
else
  printf 'CHECK config FAIL missing %s\n' "$VERIFY_CONFIG"
  printf 'RESULT FAIL\n'; exit 2
fi
printf 'CHECK config PASS %s\n' "$VERIFY_CONFIG"

# ---------- helpers ----------
FAILS=0; PASSES=0; SKIPS=0
sha256_cmd() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$@"; else shasum -a 256 "$@"; fi
}
is_skipped() { [[ " $VERIFY_SKIP " == *" $1 "* ]]; }
# run_check NAME CMD... : runs CMD in a subshell, logs output, prints one structured line.
run_check() {
  local name="$1"; shift
  local log="$VERIFY_LOG_DIR/$name.log"
  if is_skipped "$name"; then printf 'CHECK %s SKIP (VERIFY_SKIP)\n' "$name"; SKIPS=$((SKIPS+1)); return 0; fi
  local rc=0
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
  local prefix="$1" arr="$2" i=0 cmd
  local -n ref="$arr"
  for cmd in "${ref[@]}"; do
    i=$((i+1))
    run_check "$prefix.$i" bash -o pipefail -c "$cmd"
  done
}
finish() {
  printf 'SUMMARY pass=%s fail=%s skip=%s log_dir=%s\n' "$PASSES" "$FAILS" "$SKIPS" "$VERIFY_LOG_DIR"
  if [[ $FAILS -eq 0 && $SKIPS -eq 0 ]]; then printf 'RESULT PASS\n'; printf 'DONE\n'; exit 0; fi
  if [[ $FAILS -eq 0 ]]; then printf 'RESULT PASS-WITH-SKIPS (not DONE)\n'; exit 3; fi
  printf 'RESULT FAIL\n'; exit 1
}

# ---------- built-in checks ----------
check_protected_manifest() {
  [[ -f "$VERIFY_MANIFEST" ]] || { echo "manifest missing: $VERIFY_MANIFEST"; return 1; }
  # Lines: "<sha256>  <relative path>". Every listed file must exist and match.
  local rc=0
  while IFS= read -r line; do
    [[ -z "$line" || "$line" == \#* ]] && continue
    local want="${line%% *}" path="${line#* }"; path="${path# }"
    if [[ ! -f "$path" ]]; then echo "MISSING $path"; rc=1; continue; fi
    local have; have="$(sha256_cmd "$path" | awk '{print $1}')"
    if [[ "$have" != "$want" ]]; then echo "MODIFIED $path"; rc=1; else echo "ok $path"; fi
  done <"$VERIFY_MANIFEST"
  return $rc
}

changed_files() {
  # tracked changes vs base + staged + untracked (excluding ignored)
  { git diff --name-only "$BASE_REF" --; git diff --name-only --cached --; git ls-files --others --exclude-standard; } \
    | sed '/^$/d' | sort -u
}
matches_any() {  # matches_any FILE ARRAYNAME
  local f="$1"; local -n globs="$2"; local g
  for g in "${globs[@]}"; do [[ "$f" == $g ]] && return 0; done
  return 1
}
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
  local rc=0 entry regex paths
  for entry in "${FORBIDDEN_PATTERNS[@]}"; do
    regex="${entry%%|*}"; paths="${entry#*|}"; [[ "$paths" == "$entry" ]] && paths=""
    local hits
    if [[ -n "$paths" ]]; then
      # shellcheck disable=SC2086
      hits="$(git grep -n -E -e "$regex" -- $paths || true)"
    else
      hits="$(git grep -n -E -e "$regex" || true)"
    fi
    if [[ -n "$hits" ]]; then echo "FORBIDDEN /$regex/ found:"; echo "$hits"; rc=1; else echo "ok /$regex/ absent"; fi
  done
  return $rc
}
check_forbidden_paths() {
  local rc=0 p
  for p in "${FORBIDDEN_PATHS[@]}"; do
    if [[ -e "$p" ]]; then echo "EXISTS $p"; rc=1; else echo "ok absent $p"; fi
  done
  return $rc
}
check_required_paths() {
  local rc=0 p
  for p in "${REQUIRED_PATHS[@]}"; do
    if [[ -e "$p" ]]; then echo "ok $p"; else echo "MISSING $p"; rc=1; fi
  done
  return $rc
}

# Progress consistency: progress.md checklist == task.md checklist modulo [ ]/[x];
# all leaves checked; parents checked iff all children checked; STATE=DONE; CURRENT=NONE.
checklist_lines() { # checklist_lines FILE -> lines between markers
  awk '/<!-- checklist:start -->/{f=1;next} /<!-- checklist:end -->/{f=0} f' "$1"
}
check_progress() {
  [[ -n "$PROGRESS_FILE" && -n "$TASK_FILE" ]] || { echo "PROGRESS_FILE/TASK_FILE unset"; return 1; }
  [[ -f "$PROGRESS_FILE" && -f "$TASK_FILE" ]] || { echo "missing $PROGRESS_FILE or $TASK_FILE"; return 1; }
  local t p
  t="$(checklist_lines "$TASK_FILE" | sed -E 's/^( *)- \[[ x]\] /\1- [ ] /')"
  p="$(checklist_lines "$PROGRESS_FILE" | sed -E 's/^( *)- \[[ x]\] /\1- [ ] /')"
  [[ -n "$t" ]] || { echo "no checklist block in $TASK_FILE"; return 1; }
  if [[ "$t" != "$p" ]]; then
    echo "checklist text/structure differs from task.md:"; diff <(echo "$t") <(echo "$p") || true; return 1
  fi
  # structural validation of progress.md checklist
  checklist_lines "$PROGRESS_FILE" | awk '
    function depth(s,  n){ n=match(s,/[^ ]/); return (n-1)/4 }
    {
      if ($0 !~ /^( {4}){0,3}- \[[ x]\] \*\*[0-9]+(\.[0-9]+){0,3}\*\* /) { print "BAD LINE: " $0; bad=1; next }
      d=depth($0); id=$0; sub(/^.*\*\*/,"",id); id=$0; match(id,/\*\*[0-9.]+\*\*/); id=substr(id,RSTART+2,RLENGTH-4)
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
  [[ "$cur" == "NONE" ]]  || { echo "CURRENT=$cur (want NONE)"; return 1; }
  echo "ok STATE=DONE CURRENT=NONE"
}

# ---------- run ----------
run_check protected_manifest check_protected_manifest
run_check scope            check_scope
run_check required_paths   check_required_paths
run_check forbidden_paths  check_forbidden_paths
run_check forbidden_patterns check_forbidden_patterns
run_cmd_list focused    FOCUSED_CMDS
run_cmd_list regression REGRESSION_CMDS
run_cmd_list lint       LINT_CMDS
if [[ -n "$PROGRESS_FILE" ]]; then run_check progress check_progress; fi
for name in "${EXTRA_CHECKS[@]}"; do run_check "$name" "check_$name"; done
finish
```

### `manifest.sh` (dispatch-time manifest generation and standalone check)

```bash
#!/usr/bin/env bash
# manifest.sh — generate / check the protected-path sha256 manifest at dispatch time.
#   manifest.sh gen  [-o protected.sha256] [-r ROOT] PATH...   # PATH may be file or dir (recursed)
#   manifest.sh check [-m protected.sha256] [-r ROOT]
# Output line format: "<sha256>  <repo-relative path>", sorted; deterministic across runs.
set -Eeuo pipefail
sha256_cmd() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$@"; else shasum -a 256 "$@"; fi; }

mode="${1:-}"; shift || true
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"; OUT=".task/protected.sha256"; MAN=".task/protected.sha256"
while getopts "o:r:m:" opt; do case "$opt" in o) OUT="$OPTARG";; r) ROOT="$OPTARG";; m) MAN="$OPTARG";; *) exit 64;; esac; done
shift $((OPTIND-1))
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
    # normalize to two-space separator and strip any leading "./"
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
```

Manifest line format: `<sha256>  <repo-relative path>`, sorted by path, LF, no `./` prefix — identical to `sha256sum -c` input, so `sha256sum -c .task/protected.sha256` is an independent cross-check. Directories are recursed. The manifest cannot list itself; the gate protects it by holding the trusted copy outside the workspace and (optionally) by listing it in `DENIED_GLOBS`.

### Dispatch procedure

```bash
# in the orchestrator, after task.md / verify.config / progress.md are generated at $WS
cd "$WS"
bash .task/manifest.sh gen -o .task/protected.sha256 \
     .task/task.md .task/verify.config verify.sh tests/fixtures tests/expired_refresh_token.rs
git add -A && git commit -qm "TASK-042: dispatch"                # BASE_REF = this commit (or main)
mkdir -p "$TRUSTED/TASK-042"
cp verify.sh .task/verify.config .task/protected.sha256 .task/task.md "$TRUSTED/TASK-042/"
# optional hardening: docker run -v "$WS:/work" -v "$TRUSTED/TASK-042/task.md:/work/.task/task.md:ro" \
#   -v "$TRUSTED/TASK-042/verify.sh:/work/verify.sh:ro" ...
```

### Gate procedure

```bash
cd /
env -i PATH="$PATH" HOME="$HOME" \
  VERIFY_ROOT="$WS" VERIFY_CONFIG="$TRUSTED/TASK-042/verify.config" \
  VERIFY_MANIFEST="$TRUSTED/TASK-042/protected.sha256" VERIFY_LOG_DIR="$RUN/verify-logs" \
  bash "$TRUSTED/TASK-042/verify.sh" | tee "$RUN/verify.out"
rc=${PIPESTATUS[0]}
[[ $rc -eq 0 && "$(tail -n1 "$RUN/verify.out")" == "DONE" ]] || mark_not_done
```

Then the semantic reviewer (layer 3 from raw/01) gets `task.md`, `git diff $BASE_REF`, `verify.out`, and `progress.md`.

### Agent-facing usage (goes into `task.md` verification contract)

```text
Run `./verify.sh` from the repository root. It prints one `CHECK <name> PASS|FAIL` line per check;
on FAIL it prints the log tail. It exits 0 and prints `DONE` as its last line only when every check
passes. Never set VERIFY_SKIP, never edit verify.sh, .task/verify.config, .task/protected.sha256,
or any path they list; the outer gate reruns a trusted copy and rejects the task if those differ.
```

### Fixture test log (this session, macOS)

| Scenario | Result |
| --- | --- |
| Fresh dispatch, no work | `protected_manifest` PASS, `required_paths` FAIL, `focused.1` FAIL, `progress` FAIL (4 unchecked leaves), `RESULT FAIL`, rc=1 |
| Work complete, progress all `[x]`, `STATE: DONE`, `CURRENT: NONE` | 9/9 PASS, `RESULT PASS`, last line `DONE`, rc=0 |
| Parent `[x]` with child `[ ]` | `PARENT CHECKED WITH UNCHECKED CHILD 2 / 2.1 / 2.1.1`, `UNCHECKED LEAF 2.1.1.1`, FAIL |
| Agent rewords a checklist line in `progress.md` | `checklist text/structure differs from task.md` + diff, FAIL |
| Weakened protected test + stray `docs.txt` + `legacy_sub` in src | `MODIFIED tests/test_add.sh`, `OUTSIDE docs.txt`, `FORBIDDEN /legacy_sub/`, FAIL |
| Agent edits `verify.sh` and regenerates manifest in workspace | inner run: scope FAIL (denied globs); outer trusted run: `MODIFIED verify.sh`, FAIL |

### Open items / UNVERIFIED

- Behaviour under Docker read-only bind mounts of single files (macOS Docker Desktop file-mount semantics differ from Linux) — UNVERIFIED.
- `SessionStart matcher=compact` / `PostCompact` context injection reliability in current Claude Code — UNVERIFIED (issue #15174 was against v2.0.76); mitigation: resume instructions duplicated into `CLAUDE.md`.
- `check_progress` currently asserts the terminal state only; a `VERIFY_PROGRESS_MODE=partial` that validates one-current-leaf / `LEAVES` arithmetic mid-run is a small follow-up.
- Codex `/goal` evaluator semantics assumed equivalent to Claude's (transcript-only) — UNVERIFIED for Codex.
- `git grep` in `check_forbidden_patterns` only sees tracked + index files; untracked new files with forbidden strings are missed unless staged. Gate can `git add -A` into a temp index before running, or switch to `grep -rE` with `--exclude-dir=.git`. Deferred.

## Sources

- https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents
- https://www.anthropic.com/engineering/harness-design-long-running-apps
- https://developers.openai.com/cookbook/articles/codex_exec_plans
- https://learn.chatgpt.com/use-cases/follow-goals
- https://code.claude.com/docs/en/goal
- https://code.claude.com/docs/en/hooks
- https://github.com/anthropics/claude-code/issues/15174
- https://github.com/anthropics/claude-code/blob/main/plugins/ralph-wiggum/README.md
- https://github.com/ghuntley/how-to-ralph-wiggum
- https://github.com/steveyegge/beads ; https://steve-yegge.medium.com/the-beads-revolution-how-i-built-the-todo-system-that-ai-agents-actually-want-to-use-228a5f9be2a9
- https://docs.github.com/en/get-started/writing-on-github/working-with-advanced-formatting/about-task-lists
- https://aider.chat/2024/08/14/code-in-json.html
- https://www.swebench.com/SWE-bench/api/harness/ ; https://openai.com/index/introducing-swe-bench-verified/
