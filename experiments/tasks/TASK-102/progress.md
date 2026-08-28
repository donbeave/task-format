TASK: TASK-102
STATE: IN_PROGRESS
CURRENT: 1.1
BASELINE: <not run>

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-003` pass — evidence: each command exits 0.
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test app_create_form_test` fails to compile (`CreateForm`/`Field` missing).
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** Form state and editing keys (`R-001`, `D-032`) — evidence: `cargo test -p pgtui --test app_create_form_test editing` → `4 passed`.
        - [ ] **2.1.1** `n` enters `Screen::CreateConnection` with a blank form focused on Name — evidence: `cargo test -p pgtui --test app_create_form_test editing_n_opens_blank_form` → `1 passed`.
        - [ ] **2.1.2** `Tab`/`BackTab` wrap through the six fields; chars append, `Backspace` pops, Port digits only — evidence: `cargo test -p pgtui --test app_create_form_test -- editing_tab editing_chars editing_port` → `3 passed`.
    - [ ] **2.2** Validation and error messages (`R-002`, `D-032`, `D-013`) — evidence: `cargo test -p pgtui --test app_create_form_test validation` → `3 passed`.
    - [ ] **2.3** Save effect, reload, cursor placement, duplicate handling, `Esc` (`R-003`, `R-004`) — evidence: `cargo test -p pgtui --test runtime_create_test` → `3 passed`.
        - [ ] **2.3.1** `runtime::execute(SaveConnection)` inserts and replies `Msg::Saved`; `Saved(Ok)` reloads and selects the row — evidence: `cargo test -p pgtui --test runtime_create_test save_roundtrip` → `1 passed`.
        - [ ] **2.3.2** `Saved(Err(DuplicateName))` keeps the form with the error status; `Esc` discards — evidence: `cargo test -p pgtui --test runtime_create_test -- duplicate_keeps_form esc_discards` → `2 passed`.
    - [ ] **2.4** Form renders per `D-060` with masked password (`R-005`) — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_create_form_test` → `6 passed`.
    - [ ] **2.5** Unit tests for `CreateForm::validate` exist in `src/app.rs` — evidence: `cargo test -p pgtui --lib -- --list | grep -c 'app::tests::validate'` prints ≥ 3.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001`, `AC-002` — evidence: `cargo test -p pgtui --test app_create_form_test` → `7 passed`.
    - [ ] **3.2** `AC-003`, `AC-004` — evidence: `cargo test -p pgtui --test runtime_create_test` → `3 passed`.
    - [ ] **3.3** `AC-005`, `AC-006` — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_create_form_test` → `6 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only the five `expected_paths` files changed, nothing temporary or unrelated — evidence: `git status --porcelain` shows only those paths.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full verifier output shown in the transcript.
<!-- checklist:end -->

## Log

## Handoff
NEXT: 1.1
CURRENT_FAILURE: none
DECISIONS: none
