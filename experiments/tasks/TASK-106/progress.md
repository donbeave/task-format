TASK: TASK-106
STATE: IN_PROGRESS
CURRENT: 1.1
BASELINE: <not run>

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-004` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test app_disconnect_test` → `d_disconnects` fails.
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Disconnect flow (`R-001`, `R-002`, `D-030`, `D-033`) — evidence: `cargo test -p pgtui --test app_disconnect_test` → `5 passed`.
        - [ ] **2.1.1** `d` emits `Effect::Disconnect`; `Disconnected` resets session state — evidence: `cargo test -p pgtui --test app_disconnect_test d_disconnects disconnected_resets` → `2 passed`.
        - [ ] **2.1.2** `Ctrl+C` while connected yields `[Disconnect, Quit]` — evidence: `cargo test -p pgtui --test app_disconnect_test ctrl_c_disconnects_then_quits` → `1 passed`.
    - [ ] **2.2** Runtime closes the PG session and sets `application_name=pgtui` (`R-001`, `D-024`) — evidence: `cargo test -p pgtui --test pg_disconnect_test` → `1 passed`.
    - [ ] **2.3** Terminal lifecycle and exit code 0 on both quit paths; panic hook restores the terminal (`R-003`, `D-040`) — evidence: `cargo test -p pgtui --test cli_exit_test` → `2 passed`.
    - [ ] **2.4** `gallery` binary renders all ten screens via `pgtui::render` (`R-004`, `R-005`, `D-080`) — evidence: `cargo test -p pgtui --test gallery_test` → `3 passed`.
        - [ ] **2.4.1** `--out <dir>` creates 10 SVG + 10 PNG files with the D-071 names — evidence: `cargo test -p pgtui --test gallery_test writes_twenty_files` → `1 passed`.
        - [ ] **2.4.2** SVGs match protected `__svg` snapshots and are stable across runs — evidence: `cargo test -p pgtui --test gallery_test svg_matches_snapshots svg_deterministic` → `2 passed`.
    - [ ] **2.5** `docs/screens/` committed output and README screen list (`R-006`) — evidence: `ls docs/screens | wc -l` → `20`; `grep -c 'docs/screens/' README.md` → `10` or more.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001`, `AC-003` — evidence: `cargo test -p pgtui --test app_disconnect_test` → `5 passed`.
    - [ ] **3.2** `AC-002`, `AC-004` — evidence: `cargo test -p pgtui --test pg_disconnect_test --test cli_exit_test` → `3 passed` in total.
    - [ ] **3.3** `AC-005`, `AC-006` — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test gallery_test --test 'screen_*'` → `25 passed` in total.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated — evidence: `git status --porcelain` and `git diff --stat baseline` show only in-scope files.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full verifier output shown in the transcript.
<!-- checklist:end -->

## Log

## Handoff
NEXT: 1.1
CURRENT_FAILURE: none
DECISIONS: none
