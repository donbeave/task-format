TASK: TASK-101
STATE: IN_PROGRESS
CURRENT: 1.1
BASELINE: <not run>

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-003` pass — evidence: each command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test store_test` fails to compile with unresolved `pgtui::store` imports.
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Turso connection store (`R-001`, `D-020..D-022`) — evidence: `cargo test -p pgtui --test store_test` exits 0.
        - [ ] **2.1.1** `ConnectionStore::open` creates the D-021 schema idempotently — evidence: `cargo test -p pgtui --test store_test open_creates_schema` → `1 passed`.
        - [ ] **2.1.2** `insert` returns the saved row and `list` orders by name — evidence: `cargo test -p pgtui --test store_test insert_then_list_sorted_by_name` → `1 passed`.
        - [ ] **2.1.3** Unique-name violation maps to `StoreError::DuplicateName` — evidence: `cargo test -p pgtui --test store_test duplicate_name_rejected` → `1 passed`.
    - [ ] **2.2** App core and key mapping (`R-002`, `D-010`, `D-011`, `D-030`, `D-031`) — evidence: `cargo test -p pgtui --test app_connection_list_test` exits 0.
        - [ ] **2.2.1** `Msg::Connections` populates the list and clamps the cursor — evidence: `cargo test -p pgtui --test app_connection_list_test connections_msg_populates_list` → `1 passed`.
        - [ ] **2.2.2** `j`/`k` clamp at both ends; `q` and `Ctrl+C` emit `Effect::Quit` — evidence: `cargo test -p pgtui --test app_connection_list_test -- clamp quit` → `4 passed`.
    - [ ] **2.3** ConnectionList and status line render per `D-060` (`R-003`) — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_connection_list_test` → `4 passed`.
    - [ ] **2.4** CLI, store path precedence and exit codes (`R-004`, `R-005`, `D-020`, `D-040`) — evidence: `cargo test -p pgtui --test cli_test` → `3 passed`.
    - [ ] **2.5** Unit tests for `keys::action_for` and store path resolution exist in `src/` — evidence: `cargo test -p pgtui --lib -- --list | grep -c 'keys::tests\|store::tests'` prints ≥ 4.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001` and `AC-002` — evidence: `cargo test -p pgtui --test store_test` → `5 passed`.
    - [ ] **3.2** `AC-003` and `AC-004` — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_connection_list_test` → `4 passed`.
    - [ ] **3.3** `AC-005` — evidence: `cargo test -p pgtui --test app_connection_list_test` → `5 passed`.
    - [ ] **3.4** `AC-006` — evidence: `cargo test -p pgtui --test cli_test` → `3 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only `crates/pgtui/src/*` changed, nothing temporary or unrelated — evidence: `git status --porcelain` shows only those paths.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full verifier output shown in the transcript.
<!-- checklist:end -->

## Log

## Handoff
NEXT: 1.1
CURRENT_FAILURE: none
DECISIONS: none
