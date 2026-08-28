TASK: TASK-000
STATE: IN_PROGRESS
CURRENT: 1.1
BASELINE: <not run>

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-NNN` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline command run and its failing result recorded in `progress.md` `BASELINE:` — evidence: `<baseline command>` output matches the expected pre-change result.
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** <Coherent unit satisfying `R-001`> — evidence: `<focused command>` exits 0.
        - [ ] **2.1.1** <Sub-step when the unit genuinely decomposes> (`R-001`) — evidence: `<command>` → `<result>`.
        - [ ] **2.1.2** <Sub-step> (`R-001`, `AC-001`) — evidence: `<command>` → `<result>`.
    - [ ] **2.2** <Coherent unit satisfying `R-002`> — evidence: `<command>` exits 0.
    - [ ] **2.3** Superseded path removed (`R-003`, `R-004`) — evidence: `<grep command>` prints nothing and regressions pass.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001` — evidence: `<AC-001 command>` → `<expected>`.
    - [ ] **3.2** `AC-002` — evidence: `<AC-002 command>` → `<expected>`.
    - [ ] **3.3** `AC-003` — evidence: `<AC-003 command>` → `<expected>`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated — evidence: `git status --porcelain` and `git diff --stat` show only in-scope files.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full verifier output shown in the transcript.
<!-- checklist:end -->

## Log

## Handoff
NEXT: 1.1
CURRENT_FAILURE: none
DECISIONS: none
