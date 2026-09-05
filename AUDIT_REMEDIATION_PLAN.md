# Task Format Readiness and Remediation Plan

## Current verdict

**NOT READY for real-use promotion.** The supplied audit's verdict is correct, but its trust analysis is incomplete. The current design has four blocking trust-boundary failures:

1. Host-side verification executes executor-controlled code outside the run container (`harness/src/cmds/gate.rs:53`, `harness/src/gate.rs:170`, `harness/src/gate.rs:506`).
2. Host Git commands operate inside the executor-owned `.git` directory (`harness/src/ops/container.rs:176`, `harness/src/cmds/promote.rs:68`). Repository-local hooks/config and replace refs can therefore affect host execution and integrity checks.
3. Promotion compares only `HEAD`, stages the later mutable worktree, and pushes the local `main` ref rather than an exact gated object (`harness/src/cmds/promote.rs:42`, `harness/src/cmds/promote.rs:68`, `harness/src/cmds/promote.rs:73`).
4. Forbidden-pattern execution errors are converted to empty output and accepted as absence (`harness/src/gate.rs:489`, `harness/src/gate.rs:493`, `harness/src/gate.rs:495`).

Do not use `taskfmt promote` for untrusted or autonomous runs until Phase 0 is complete.

Audit baseline: `71dd68032a9c5a02e7da9144186a4112d529ebcc`, verified 2026-09-05.

## Verification snapshot

| Check | Result | Meaning |
| --- | --- | --- |
| `cargo fmt --manifest-path harness/Cargo.toml --check` | PASS | Formatting is clean. |
| `cargo test --manifest-path harness/Cargo.toml` | PASS, 351 tests | Existing unit/integration suite is green. It does not cover the blocking trust paths. |
| `cargo test --manifest-path harness/Cargo.toml --test gate_tamper_matrix` | PASS, 19 tests | Existing tamper cases pass; the matrix accepts completed progress with no log evidence (`harness/tests/gate_tamper_matrix.rs:78`). |
| `cargo run --quiet --manifest-path harness/Cargo.toml -- selftest` | FAIL, 1 case | The C3 mutant is stale, not proof the real lint rule is broken (`harness/src/selftest.rs:437`, `harness/tests/lint_corpus.rs:371`). |
| `cargo clippy --manifest-path harness/Cargo.toml --all-targets -- -D warnings` | FAIL | `harness/src/acceptance.rs:654` triggers `clippy::needless_borrow`. |
| Source-built `taskfmt lint` | PASS with warnings on all seven tasks | All READMEs exceed the stated 10,000-byte target; TASK-001..003 also have oracle warnings. |

Use the source-built binary for this repository. The installed `taskfmt` was observed to have a different fingerprint and reject current packages.

## Audit corrections

### Confirmed

- Gate checks progress boxes and four headers, not log grammar or checkbox/latest-event equivalence (`harness/src/gate.rs:569`, `harness/src/gate.rs:625`, `harness/src/progress.rs:104`).
- Direct gate validates typed acceptance shape, not the full cross-file task contract (`harness/src/gate.rs:269`).
- Missing AC-to-command coverage is warning-only; non-Cargo matching scans serialized TOML text (`harness/src/lint.rs:1488`, `harness/src/lint.rs:1512`).
- `Expected` is required prose but is not evaluated (`harness/src/acceptance.rs:454`, `harness/src/gate.rs:506`).
- Required/pattern paths lack repository-containment validation (`harness/src/gate.rs:442`, `harness/src/gate.rs:480`).
- Selfcheck is opt-in (`harness/README.md:77`).
- Task frontmatter and experiment config silently ignore unknown fields, while `verify/v1` rejects them (`harness/src/taskfile.rs:64`, `harness/src/config.rs:36`, `harness/src/verifycfg.rs:11`).
- TASK-004 contradicts its predecessor and itself about `db/` (`experiments/tasks/TASK-004/README.md:33`, `experiments/tasks/TASK-004/README.md:53`, `experiments/tasks/TASK-004/README.md:233`).
- TASK-006's baseline command cannot prove the claimed unreachable screen; its database test does not exercise screen entry (`experiments/tasks/TASK-006/README.md:50`, `experiments/tasks/TASK-006/trusted/crates/pgtui/tests/pg_custom_sql_test.rs:5`).
- Current experiment orchestration is a sequential lifecycle runner, not a controlled format comparison (`harness/src/cmds/experiment.rs:90`, `harness/src/cmds/experiment.rs:181`).

### Qualified

- Scope has three descriptions: prose Scope, frontmatter `expected_paths`, and `verify.toml.allowed_globs`. Standard dispatch lint reconciles the two machine lists, so this is a maintenance and direct-gate defect, not silent drift on every dispatched run (`harness/src/lint.rs:1441`).
- Checklist/AC/config have distinct roles, but literal command and result duplication is still an avoidable synchronization defect.
- The 5–20 leaf rule, no-single-child rule, large tasks, and cumulative decisions are measurable facts. Claims that they harm executor performance remain hypotheses until controlled comparisons exist (`README.md:82`).
- TASK-007 is broad. Whether it violates “one bounded outcome” depends on whether a release-completion slice is allowed.
- Missing experimental comparisons blocks claims of format effectiveness. It is not, alone, an operational readiness failure; the trust defects are.

## Readiness criteria

| Criterion | Current | Required proof |
| --- | --- | --- |
| Untrusted task code cannot execute with host privileges | FAIL | Verifier runs in an isolated, credential-free sandbox; adversarial escape/canary tests pass. |
| Executor-controlled Git metadata is outside the trust boundary | FAIL | Host never uses the executor's `.git`; hook/config/replace-ref tests pass. |
| Promoted tree exactly equals verified tree | FAIL | Gate records immutable tree/parent; promotion pushes a commit built from that tree with an explicit refspec and lease. |
| Gate fails closed on every verifier error | FAIL | Invalid regex, unreadable/missing path, tool spawn error, and non-0/1 pattern status fail. |
| Full task contract is enforced at gate time | FAIL | Malformed README, cross-file mismatch, empty verifier, and unknown keys fail direct gate. |
| Every requirement maps to executed proof | FAIL | ACs reference stable check IDs; missing/duplicate references are fatal. |
| Declared expected results are executable | FAIL | Exit/stdout/stderr/count matchers are evaluated and recorded. |
| Baseline distinguishes absent from present behavior | FAIL | Mandatory base-red and reference-green selfcheck for promotable runs. |
| Progress representation is internally consistent | FAIL | Versioned strict parser; known IDs/statuses; latest event and derived state agree. Progress remains coordination state, not trust evidence. |
| Package semantics are consistent | FAIL | Corrected corpus passes semantic review and executable predecessor/baseline checks. |
| Author diagnostics identify exact source | FAIL | Every finding includes package path and line; batch and JSON output tested. |
| Controlled evidence supports format claims | FAIL, research-only | Frozen paired variants, repeats, assignment, metrics, and durable redacted records exist. |
| Repository gates are green | FAIL | fmt, Clippy, all tests, selftest, corpus lint, and enabled end-to-end tests pass. |

**OPERATIONALLY READY** requires every criterion except the research-only controlled-evidence row. **FORMAT EFFECTIVENESS VALIDATED** additionally requires controlled-study evidence. Do not collapse these labels.

## Recommended direction

Choose the **moderate structural revision**, after a mandatory security-boundary repair.

Keep Markdown as the human execution contract. Move machine authority for scope, checks, commands, expected matchers, and lineage into one strict registry. Generate or validate the repeated Markdown views. Preserve frozen legacy readers and snapshots; never reinterpret old records.

Strongest objection: authoring becomes dependent on resolver/generator tooling. Mitigation: commit a fully resolved, readable task snapshot for every run and make `taskfmt render --check` deterministic.

## Implementation plan

### Phase 0 — Rebuild the trust boundary (P0)

**Files**

- `harness/src/ops/container.rs`
- `harness/src/ops/docker.rs`
- `harness/src/ops/git.rs`
- New: `harness/src/ops/snapshot.rs`
- `harness/src/gate.rs`
- `harness/src/cmds/gate.rs`
- `harness/src/cmds/container_entrypoint.rs`
- `harness/src/cmds/promote.rs`
- `harness/src/cmds/run.rs`
- `harness/src/runstate.rs`
- `harness/src/fingerprint.rs`
- New: `harness/images/verifier/Dockerfile`
- `harness/tests/gate_tamper_matrix.rs`
- New: `harness/tests/host_boundary.rs`
- New: `harness/tests/promotion_integrity.rs`

**Changes**

1. Add `run/v1` and a versioned secure gate record before restoring promotion. Missing-schema records remain legacy/inspectable and are never accepted as secure records.
2. Stop the executor/container before snapshotting. Route manual and automatic gate paths through the same quiesce-and-snapshot function.
3. Mirror the workspace content without `.git` into a fresh trusted clone rooted at the recorded base. This is a full replacement: executor-deleted base files stay deleted. Do not follow symlinks during the mirror. Never run host Git in the executor checkout.
4. Build an immutable candidate tree in the trusted clone, then gate a fresh checkout of that exact tree. Ignored or unstaged content cannot influence checks without entering the candidate tree.
5. Record tree OID, expected parent, task snapshot hash, verifier hash, harness fingerprint, verifier image digest, terminal state, and gate-evidence digest in the versioned record.
6. Copy task/progress inputs into host-owned staging after rejecting symlinks and non-regular files. Write gate logs only to a new host-owned directory that was never writable by the executor; do not reuse `/out`.
7. Run all verification commands inside a locked verifier container/VM, not as the host user. Mount only the immutable candidate, trusted staged inputs, and host-owned output directory. No host credentials, agent home, Docker socket, or unrestricted network.
8. Replace Docker-socket-dependent task tests with a harness-owned prerequisite service on an isolated network. Pass connection details as data.
9. Promote from the trusted clone only. Create the commit from the recorded tree and parent, then push `<recorded-commit>:refs/heads/main` with an expected-old-value lease. Never use `git add -A` or implicit `main` from the executor clone.
10. Sanitize Git environment/config as defense in depth: disable hooks, replace objects, external helpers, filesystem monitors, and repository-controlled transport settings.
11. A gate created by an older/incomplete record schema is inspectable but not promotable; require re-gating through the secure path.

**Tests**

- Repository-local hook and Git-config canaries never execute on host.
- Replace refs cannot hide changed files.
- Diverting local `main` cannot change the pushed object.
- Tracked/untracked mutation during or after snapshot cannot change the gated/promoted tree.
- Symlink and path traversal cannot expose host files.
- Gate commands cannot read a host canary, credentials, agent home, or Docker socket.
- Executor-created symlinks in progress, snapshot, or output paths are rejected and cannot redirect host writes.
- Concurrent remote `main` change causes lease refusal.

**Exit condition**

No host process executes executor-controlled code or consumes executor-controlled Git metadata. Promotion proves exact tree equality.

**Rollback**

Disable `gate`/`promote` for autonomous runs. Do not fall back to the current host gate.

### Phase 1 — Make the current gate fail closed (P0/P1)

**Files**

- `harness/src/gate.rs`
- `harness/src/verifycfg.rs`
- `harness/src/lint.rs`
- `harness/src/cmds/gate.rs`
- `harness/tests/gate_tamper_matrix.rs`
- `harness/tests/lint_corpus.rs`
- `harness/src/selftest.rs`

**Changes**

1. Interpret pattern status exactly: `0` means forbidden hit, `1` means absent, every other status/spawn failure means gate failure. Preserve stderr in evidence.
2. Add one repository-relative path type. Reject absolute paths, `..`, empty components, platform prefixes, and resolved symlink escape for every path-bearing field.
3. Run full package lint inside direct verify/gate. Missing README, mixed/ambiguous acceptance dialects, semantically empty verifier, and cross-file mismatch must fail. Preserve explicitly detected legacy-table `task/v4` behavior; apply stricter grammar only to v5.
4. Make `--fail-fast` stop the entire ordered check pipeline; callers must propagate the stop signal.
5. Require promotable terminal state in promotion logic, not only in the experiment wrapper.
6. Fix the stale selftest C3 mutation and Clippy failure before accepting further format work.

**Tests**

- Invalid regex, missing pattern path, grep/tool error, absolute required path, traversal, and symlink escape fail.
- README with no task contract fails direct gate.
- `schema = "verify/v1"` with empty effective checks fails.
- A focused failure prevents every later check under `--fail-fast`.
- Direct promotion refuses non-promotable terminal states.
- Selftest mutation is checked against the current typed Markdown syntax.

**Exit condition**

All current-format gate errors fail closed; fmt, Clippy, tests, selftest, and corpus lint complete without unexpected warnings.

**Rollback**

Path and gate hardening are one-way correctness fixes. Rollback means disable affected operations, not restore fail-open behavior.

### Phase 2 — Introduce single machine authority (P1)

**Files**

- `harness/src/taskfile.rs`
- `harness/src/acceptance.rs`
- `harness/src/verifycfg.rs`
- `harness/src/lint.rs`
- `harness/src/gate.rs`
- `harness/src/progress.rs`
- `harness/src/runstate.rs`
- `harness/src/cli.rs`
- `harness/src/cmds/progress_init.rs`
- New: `harness/src/cmds/migrate.rs`
- `reference/task-template/README.md`
- `reference/task-template/verify.toml`
- `reference/task-template/AGENTS.md`

**Schemas**

- Add `task/v5`, `verify/v2`, `progress/v1`, strict authored `experiment/v2`, and `experiment-state/v1`. `run/v1` already exists from Phase 0.
- Freeze `task/v4`, `verify/v1`, and `experiment/v1` readers. Preserve every historically emitted v4 acceptance profile with golden fixtures: legacy table, earlier typed metadata-first blocks, and current behavior-first blocks. Detect the historical profile or use recorded harness provenance; never reinterpret it through the newest parser.
- Missing-schema run/progress files use an explicit legacy reader. Legacy records cannot be promoted without secure re-gating.

**Authority model**

1. `README.md` owns goal, context, behavioral scope, requirements, acceptance behavior, and task-specific fixed decisions.
2. `verify/v2` owns one writable-path list, predecessor/base identity, preconditions, and a registry of stable check IDs.
3. Each check defines `argv` by default, an explicit shell escape hatch when required, phase, covers, and executable matchers for exit/stdout/stderr/count/artifacts.
4. Acceptance criteria reference check IDs. Missing, duplicate, unused, or non-covering IDs are fatal. Remove serialized-TOML substring matching.
5. Checklist leaves reference work/AC/check IDs without copying shell commands or expected prose. Remove the mandatory 5–20 range and single-child prohibition unless later experiments support them.
6. Progress becomes strict versioned events keyed by known leaf IDs. Derive current/checklist state from events. Treat progress as coordination state only; host gate logs remain completion evidence.
7. Add strict unknown/duplicate-key rejection to new authored schemas. In legacy schemas, warn without changing historical interpretation.
8. Record gate-time schema, hashes, engine fingerprint, image digest, immutable tree, evidence digest, and normalized completion claim.
9. Add `taskfmt migrate --dry-run` and deterministic `taskfmt render --check`. Never rewrite stored run snapshots.

**Tests**

- Round-trip and golden tests for every new schema.
- Legacy readers retain frozen behavior for every historical v4 acceptance profile.
- Unknown/duplicate keys fail new schemas.
- Impossible expected matcher fails even when command exits zero.
- Command text in comments/other fields cannot satisfy AC coverage.
- Progress rejects duplicate headers, malformed/out-of-section rows, unknown IDs/statuses, and inconsistent derived state.
- Generated Markdown is byte-stable and self-contained.

**Exit condition**

One machine authority exists for scope and proof. Every requirement reaches an executed, evaluated check through stable IDs.

**Rollback**

Disable new writers/commands while retaining every new reader. Never reinterpret or overwrite v4/v1 snapshots. Rollback is feature disable on a compatible binary, never downgrade to a binary that cannot read stored v5/run-v1 artifacts.

### Phase 3 — Repair authoring UX and corpus semantics (P1/P2)

**Files**

- `harness/src/lint.rs`
- `harness/src/cmds/lint.rs`
- `harness/src/acceptance.rs`
- `harness/src/taskfile.rs`
- `harness/README.md`
- `README.md`
- `experiments/README.md`
- New: `experiments/tasks-v5/TASK-001/README.md`
- New: `experiments/tasks-v5/TASK-001/verify.toml`
- New: `experiments/tasks-v5/TASK-001/decisions.md`
- New: `experiments/tasks-v5/TASK-002/README.md`
- New: `experiments/tasks-v5/TASK-002/verify.toml`
- New: `experiments/tasks-v5/TASK-002/decisions.md`
- New: `experiments/tasks-v5/TASK-003/README.md`
- New: `experiments/tasks-v5/TASK-003/verify.toml`
- New: `experiments/tasks-v5/TASK-003/decisions.md`
- New: `experiments/tasks-v5/TASK-004/README.md`
- New: `experiments/tasks-v5/TASK-004/verify.toml`
- New: `experiments/tasks-v5/TASK-004/decisions.md`
- New: `experiments/tasks-v5/TASK-005/README.md`
- New: `experiments/tasks-v5/TASK-005/verify.toml`
- New: `experiments/tasks-v5/TASK-005/decisions.md`
- New: `experiments/tasks-v5/TASK-006/README.md`
- New: `experiments/tasks-v5/TASK-006/verify.toml`
- New: `experiments/tasks-v5/TASK-006/decisions.md`
- New: `experiments/tasks-v5/TASK-007/README.md`
- New: `experiments/tasks-v5/TASK-007/verify.toml`
- New: `experiments/tasks-v5/TASK-007/decisions.md`
- New: `experiments/decisions/registry.md`
- `experiment.toml`

**Changes**

1. Add `path:line:column` to every finding, task-labelled batch output, and stable JSON diagnostics.
2. Restrict AC parsing to `## Acceptance criteria` in v5. Reject mixed typed/legacy dialects.
3. Generate pinned task-local decision subsets from an immutable registry. Include source digest; exclude future/inactive clauses.
4. Preserve current `experiments/tasks/` as the v4 lifecycle corpus. Create corrected v5 packages in `experiments/tasks-v5/`; point new runs there only after equivalence checks.
5. Correct TASK-004 baseline and D-026 activation, narrow the tokio-postgres `Client::query` prohibition, replace TASK-006 baseline with a screen-entry oracle, and fix TASK-007's overclaimed prerequisite.
6. Split positive requirements from `MUST NOT` lists. Define predecessor task/tree identity explicitly.
7. Remove repeated acceptance grammar, protocol pointers, generic stop prose, clean-tree preconditions, and repeated literal evidence commands from generated task views.
8. Rename current globs “taskfmt path patterns.” Change semantics only under `verify/v2`.
9. Make the shipped corpus zero-warning. Treat TASK-007 breadth as an owner policy decision, not an automatic defect.

**Tests**

- Diagnostic snapshots include package and source location.
- Each lifecycle baseline fails for the documented reason and each reference solution passes.
- Decision snapshot generation is deterministic and rejects missing/inactive IDs.
- v4 and v5 packages resolve to equivalent outcome/scope/proof before switching defaults.

**Exit condition**

Canonical packages are internally consistent, locally readable, source-locatable, and warning-free.

**Rollback**

Switch `tasks_dir` back to frozen `experiments/tasks/` using a binary that still reads v5/run-v1 artifacts. Retain v5 artifacts for diagnosis; never mutate old snapshots and never downgrade to an incompatible binary.

### Phase 4 — Separate lifecycle runs from format studies (P1/P3)

**Files**

- `harness/src/config.rs`
- `harness/src/cli.rs`
- `harness/src/cmds/experiment.rs`
- `harness/src/cmds/status.rs`
- `harness/src/runstate.rs`
- `harness/src/fingerprint.rs`
- New: `harness/src/cmds/study.rs`
- New: `experiments/studies/`
- `experiment.toml`
- `README.md`
- `experiments/README.md`
- `harness/README.md`

**Changes**

1. Keep current sequential behavior as explicit `lifecycle` mode.
2. Add strict `study/v1`: case ID, invariant outcome/verifier, variants, repeats, blocks, random seed, primary endpoint, exclusions, artifact policy.
3. Run every observation from the same immutable base and trusted-suite hash. Never promote study observations into a shared baseline.
4. Add `observation/v1`: assignment, repeat, package/base/tree hashes, harness/image/agent identities, normalized executor claim, gate result, duration, scope violations, retry/rework counts, and defined diff metrics.
5. Normalize completion-claim extraction across supported agents.
6. Make disabled end-to-end tests report SKIP, not PASS. Require enabled real-container coverage in release CI.
7. Store durable redacted summaries and artifact hashes. Do not claim format effectiveness until repeated paired results exist.

**Tests**

- Deterministic balanced assignment and repeat identity.
- Same-base/same-verifier enforcement.
- Stop/failure cannot censor unrelated observations silently.
- Complete observation record and claim-vs-gate classification.
- Release CI proves end-to-end test bodies executed.

**Exit condition**

Lifecycle reliability and causal format evidence are separate, named, and mechanically enforced.

**Rollback**

Disable `study` commands. Lifecycle mode and stored observation records remain readable.

### Phase 5 — Final readiness gate

Run, in order:

1. `cargo fmt --manifest-path harness/Cargo.toml --check`
2. `cargo clippy --manifest-path harness/Cargo.toml --all-targets -- -D warnings`
3. `cargo test --manifest-path harness/Cargo.toml`
4. `cargo run --quiet --manifest-path harness/Cargo.toml -- selftest`
5. Source-built `taskfmt lint` over every active package; require zero errors and zero unexplained warnings.
6. Host-boundary and promotion-integrity adversarial suites.
7. Enabled container end-to-end suite with an assertion that test bodies ran.
8. Migration/render determinism and legacy-read tests.
9. One disposable lifecycle run: base-red, reference-green, executor run, secure gate, exact-tree promotion.

Declare **READY** only after all readiness criteria pass and the disposable promotion's remote tree equals the recorded gated tree.

## Competing directions

### 1. Minimal compatible revision

Fix fail-open patterns, path validation, direct-gate lint, progress grammar, diagnostics, selftest/Clippy, and corpus contradictions without changing authority layout.

Strongest argument against: it does not remove the host-execution/Git trust boundary or command/scope duplication. Alone, it cannot make promotion ready.

### 2. Moderate structural revision — recommended

First isolate verification and promotion. Then add stable check IDs, executable matchers, one scope registry, versioned records/progress, filtered decision snapshots, and separate study mode while keeping Markdown human-authored.

Strongest argument against: resolver/generator and secure verifier infrastructure become required dependencies.

### 3. Clean next-version design

Make one typed task graph own goal, requirements, decisions, scope, checks, dependencies, and progress; generate all Markdown and run views.

Strongest argument against: largest migration, weakest direct hand-authoring, and highest risk of replacing understandable prose with tooling complexity.

## Bounded delivery sequence

1. **Security stopgap:** disable unsafe promotion; add failing adversarial tests.
2. **Trusted snapshot and sandbox:** implement Phase 0; restore promotion only after exact-tree and host-canary proofs.
3. **Fail-closed v1:** implement Phase 1; make all repository gates green.
4. **Version boundary:** freeze legacy readers and add provenance schemas before changing semantics.
5. **Single authority:** implement `verify/v2` IDs/matchers/scope and strict progress.
6. **Corpus migration:** create and equivalence-check v5 packages; keep v4 immutable.
7. **Research mode:** add paired studies only after operational trust is restored.

Each step should be independently reviewable and leave unsafe promotion disabled until its full exit condition passes.

## Owner decisions required

1. Confirm the threat model: may the executor and all workspace content, including `.git` and build scripts, be malicious? Recommended: **yes**.
2. Select verifier isolation: rootless container, microVM, or dedicated worker. It must not mount the host Docker socket or credentials.
3. Define the verifier network allowlist and prerequisite-service interface.
4. Choose canonical authoring authority: Markdown behavior plus structured proof registry (recommended), or a fully structured task graph.
5. Require reference-green selfcheck for every promotable task, or define a non-promotable experimental class for tasks without an oracle.
6. Choose matcher scope: exit/output/count/artifact only, or richer structured test protocols.
7. Decide whether old progress/run artifacts must remain directly readable or only inspectable through legacy commands.
8. Define whether a multi-concern release slice such as TASK-007 is a valid task unit.
9. Define durable study artifact retention and redaction policy.
