TASK: TASK-105
STATE: IN_PROGRESS
CURRENT: 1.1
BASELINE: <not run>

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-004` pass — evidence: each listed command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test app_custom_sql_test` → `navigation_x_opens_sql_screen` fails (`screen == Browser`).
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Screen transitions and input editing (`R-001`, `R-002`, `D-034`) — evidence: `cargo test -p pgtui --test app_custom_sql_test navigation enter_` → `5 passed`.
    - [ ] **2.2** Custom query execution semantics (`R-003`, `D-025`) — evidence: `cargo test -p pgtui --test pg_custom_sql_test` → `4 passed`.
        - [ ] **2.2.1** Trailing `;` stripped; second row description → `DbError::MultiStatement` — evidence: `cargo test -p pgtui --test pg_custom_sql_test multi_statement_rejected trailing_semicolon` → `2 passed`.
        - [ ] **2.2.2** `CommandComplete` → info status and `sql_grid = None`; rows capped at 500 with info — evidence: `cargo test -p pgtui --test pg_custom_sql_test command_complete capped_` → `2 passed`.
    - [ ] **2.3** Custom grid is plain: no sort, no column cursor (`R-004`) — evidence: `cargo test -p pgtui --test app_custom_sql_test no_sort_in_custom_grid` → `1 passed`.
    - [ ] **2.4** CustomSql screen renders per `D-060` (`R-005`) — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_custom_sql_test` → `4 passed`.
    - [ ] **2.5** Unit test for `strip_trailing_semicolon` exists in `src/db/mod.rs` — evidence: `cargo test -p pgtui --lib db::tests::strip_trailing` → `1 passed` or more.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001`, `AC-002`, `AC-004` — evidence: `cargo test -p pgtui --test app_custom_sql_test` → `6 passed`.
    - [ ] **3.2** `AC-003` — evidence: `cargo test -p pgtui --test pg_custom_sql_test` → `4 passed`.
    - [ ] **3.3** `AC-005` — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_custom_sql_test` → `4 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated — evidence: `git status --porcelain` and `git diff --stat baseline` show only in-scope files.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full verifier output shown in the transcript.
<!-- checklist:end -->

## Log

## Handoff
NEXT: 1.1
CURRENT_FAILURE: none
DECISIONS: none
