TASK: TASK-103
STATE: IN_PROGRESS
CURRENT: 1.1
BASELINE: <not run>

<!-- checklist:start -->
- [ ] **1** Baseline is reproduced.
    - [ ] **1.1** Preconditions `P-001..P-004` pass — evidence: each command exits 0 (incl. `docker info`).
    - [ ] **1.2** Baseline failure recorded in `progress.md` `BASELINE:` — evidence: `cargo test -p pgtui --test pg_connect_test` fails to compile (`pgtui::db::postgres` missing).
- [ ] **2** Required behavior is implemented.
    - [ ] **2.1** PostgreSQL session (`R-001`, `R-006`, `D-023..D-025`) — evidence: `cargo test -p pgtui --test pg_connect_test` → `3 passed`.
        - [ ] **2.1.1** `PgSession::connect` builds the D-024 config, uses `NoTls`, times out at 5 s — evidence: `cargo test -p pgtui --test pg_connect_test refused_port_errors_fast` → `1 passed`.
        - [ ] **2.1.2** `list_tables` runs the D-025 query via `simple_query` in server order — evidence: `cargo test -p pgtui --test pg_connect_test -- lists_seed_tables fake_tables_match_pg` → `2 passed`.
    - [ ] **2.2** App handles `Connected` and sidebar keys (`R-002..R-004`, `D-033`) — evidence: `cargo test -p pgtui --test app_browser_test` → `5 passed`.
        - [ ] **2.2.1** `Enter` on the list emits `Effect::Connect(selected)`; `Connected(Ok)` enters Browser with cursor 0, Sidebar focus — evidence: `cargo test -p pgtui --test app_browser_test enter_connect` → `2 passed`.
        - [ ] **2.2.2** `Connected(Err)` stays on the list with `Status::Error`; `j`/`k` clamp in the sidebar — evidence: `cargo test -p pgtui --test app_browser_test -- connect_error sidebar_` → `3 passed`.
    - [ ] **2.3** `runtime::execute(Connect)` connects, lists, stores the session, replies (`R-001`, `D-012`) — evidence: `cargo test -p pgtui --test pg_runtime_connect_test` → `1 passed`.
    - [ ] **2.4** Browser renders sidebar + empty body + focus marker per `D-060` (`R-005`) — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_browser_test` → `2 passed`.
    - [ ] **2.5** Unit test for `quote_ident` exists in `src/db/mod.rs` — evidence: `cargo test -p pgtui --lib db::tests::quote_ident` → `≥ 1 passed`.
- [ ] **3** Acceptance criteria are proven.
    - [ ] **3.1** `AC-001`, `AC-002`, `AC-006` — evidence: `cargo test -p pgtui --test pg_connect_test` → `3 passed`.
    - [ ] **3.2** `AC-003`, `AC-004` — evidence: `cargo test -p pgtui --test app_browser_test --test pg_runtime_connect_test` → `6 passed` across both binaries.
    - [ ] **3.3** `AC-005` — evidence: `INSTA_UPDATE=no INSTA_FORCE_PASS=0 cargo test -p pgtui --test screen_browser_test` → `2 passed`.
- [ ] **4** Gate passes.
    - [ ] **4.1** Diff reviewed: only `expected_paths` changed, nothing temporary or unrelated — evidence: `git status --porcelain` shows only those paths.
    - [ ] **4.2** `/task/verify.sh` exits 0 with last line `DONE` — evidence: full verifier output shown in the transcript.
<!-- checklist:end -->

## Log

## Handoff
NEXT: 1.1
CURRENT_FAILURE: none
DECISIONS: none
