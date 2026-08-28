TASK: TASK-104
STATE: IN_PROGRESS
CURRENT: 1.1
BASELINE: <not run>

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-004` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test grid_sort_test` fails to compile (`pgtui::grid` missing).
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Grid model and sorting (`R-002`, `R-003`, `D-050..D-053`) — evidence: `cargo test -p pgtui --test grid_sort_test` → `6 passed`.
        - [ ] **2.1.1** Numeric-vs-string column detection and comparator — evidence: `cargo test -p pgtui --test grid_sort_test numeric_ string_` → `2 passed`.
        - [ ] **2.1.2** NULL placement and stable ties — evidence: `cargo test -p pgtui --test grid_sort_test nulls_ stable_` → `3 passed`.
        - [ ] **2.1.3** Unit tests for the comparator in `src/grid.rs` — evidence: `cargo test -p pgtui --lib grid::tests` → `3 passed` or more.
    - [ ] **2.2** `PgSession::query` returns an all-text `ResultSet` and preview SQL matches `D-025` (`R-001`) — evidence: `cargo test -p pgtui --test pg_preview_test` → `3 passed`.
    - [ ] **2.3** App preview flow and grid keys (`R-004`, `R-006`, `D-033`) — evidence: `cargo test -p pgtui --test app_preview_test` → `9 passed`.
        - [ ] **2.3.1** `Enter` emits `Effect::Query { Preview }`; `QueryDone(Ok)` installs the grid and focuses it — evidence: `cargo test -p pgtui --test app_preview_test enter_ query_ok` → `3 passed`.
        - [ ] **2.3.2** `h`/`l`/`j`/`k` clamp; `s` cycles; `Tab` toggles focus only with a grid — evidence: `cargo test -p pgtui --test app_preview_test cursor_ sort_cycle tab_` → `5 passed`.
    - [ ] **2.4** Grid widget and main-pane title per `D-060` (`R-005`) — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_preview_test` → `6 passed`.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001` — evidence: `cargo test -p pgtui --test grid_sort_test` → `6 passed`.
    - [ ] **3.2** `AC-002` — evidence: `cargo test -p pgtui --test pg_preview_test` → `3 passed`.
    - [ ] **3.3** `AC-003`, `AC-004`, `AC-006` — evidence: `cargo test -p pgtui --test app_preview_test` → `9 passed`.
    - [ ] **3.4** `AC-005` — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_preview_test` → `6 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated — evidence: `git status --porcelain` and `git diff --stat baseline` show only in-scope files.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full verifier output shown in the transcript.
<!-- checklist:end -->

## Log

## Handoff
NEXT: 1.1
CURRENT_FAILURE: none
DECISIONS: none
