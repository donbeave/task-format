# Task Format Finalization Plan

## Product decisions

This repository is a research product. Only the latest state on `main` matters.

- User provides sandbox or isolation boundary.
- `taskfmt` may run verification and Git directly inside that boundary.
- Verification requires network access.
- Executor workspace and Git metadata are trusted at this layer.
- Do not add nested verifier containers, rootless isolation, credential stripping, or network denial.
- Do not preserve old schemas, records, parsers, or behavior.
- Do not add legacy readers, migrations, compatibility modes, feature flags, or dual paths.
- Replace obsolete formats directly and delete superseded code.
- Commit and push verified slices directly to `main`.

These decisions are final. Implementation needs no further architecture approval.

## Current state

Completed:

- Forbidden-pattern errors fail closed.
- Repository-relative paths reject absolute paths, traversal, empty values, and symlink escapes.
- `--fail-fast` stops the complete ordered check pipeline.
- Direct verification runs full task-package lint.
- Stale selftest and Clippy failures are fixed.
- TASK-004 and TASK-006 baseline contradictions are corrected.
- PostgreSQL protocol guards cover all forbidden client methods.
- Host gate is enabled in the caller-provided sandbox.

Not complete:

- Promotion remains disabled.
- Gate records do not bind a complete immutable candidate tree.
- Task authority remains duplicated across Markdown and TOML.
- Expected-result prose is not executable.
- Progress parsing remains permissive and internally redundant.
- Diagnostics lack stable source locations and JSON output.
- Task corpus still has warnings and uses the duplicated format.
- Lifecycle execution and format studies are not separate modes.

Product is not finalized until every exit condition below passes.

## Final architecture

### Human contract

`README.md` owns task identity, title, goal, context, behavioral scope, requirements, acceptance behavior, fixed decisions, and concise work checklist. Markdown remains readable without a generator.

### Machine contract

`verify.toml` is sole machine authority for schema version, writable paths, base and predecessor identity, preconditions, stable check IDs, commands, phases, requirement coverage, and expected matchers.

Remove `expected_paths` and other duplicated machine fields from README frontmatter.

### References

- Acceptance criteria reference stable check IDs.
- Each check declares requirements it proves.
- Checklist leaves reference requirement, acceptance, and check IDs without copying commands or expected results.
- Missing, duplicate, unknown, unused, or non-covering references are fatal.

### Progress

Progress is coordination state, never completion evidence. Gate output is completion evidence.

- Use one strict versioned event format.
- Events reference known checklist leaf IDs.
- Derive checklist state from events.
- Header state, current item, latest event, and derived state must agree.
- Unknown IDs, statuses, duplicate headers/events, malformed rows, and invalid transitions fail.

### Execution modes

- `lifecycle` runs sequential tasks and may promote exact gated trees.
- `study` compares format variants from the same immutable base and verifier.
- Study observations never promote into shared lifecycle state.

## Phase 1 — Exact-tree gate and promotion

### Changes

1. Stop executor before gating.
2. Stage complete candidate once and write its Git tree object.
3. Record candidate tree OID and exact parent before verification.
4. Run verification against that frozen candidate.
5. Recompute complete tree after verification; any mutation aborts gate.
6. Record terminal state, tree, parent, task hash, verifier hash, harness fingerprint, evidence digest, verdict, and timestamps.
7. Require promotable terminal state inside `promote`.
8. Create promotion commit directly from recorded tree and parent.
9. Include provenance, DCO signoff, and agent attribution.
10. Push `<recorded-commit>:refs/heads/main` with expected-parent lease.
11. Never stage a later worktree or push an implicit local branch during promotion.
12. Refuse incomplete records, failed gates, changed candidates, and concurrent remote changes.

### Tests

- Tracked, staged, unstaged, ignored, untracked, and deleted files enter candidate exactly once.
- Mutation during or after gate cannot change promoted tree.
- Moving local `main` cannot change pushed commit.
- Non-promotable terminal states are rejected.
- Concurrent remote `main` changes cause lease refusal.
- Remote tree equals recorded gated tree.

### Exit condition

Gate and promotion operate on one immutable tree and exact parent. Enable `taskfmt promote` only after these tests pass.

## Phase 2 — One strict task schema

### Changes

1. Define one current task schema and one current verifier schema.
2. Delete old schema readers and acceptance dialects.
3. Parse README frontmatter strictly; reject unknown and duplicate keys.
4. Restrict acceptance parsing to `## Acceptance criteria`.
5. Reject mixed or ambiguous acceptance forms.
6. Define one writable-path list in `verify.toml`.
7. Define stable check IDs and phases: precondition, focused, regression, lint, gate.
8. Use argv arrays by default. Permit shell only through explicit `shell` field.
9. Connect requirements, acceptance criteria, checklist leaves, and checks through IDs.
10. Delete serialized-TOML substring matching and command-duplication rules.
11. Reject semantically empty verifier configurations.
12. Keep rendering deterministic where generated views remain necessary.

### Tests

- Unknown and duplicate keys fail.
- Missing, duplicate, unused, and uncovered IDs fail.
- Comments and unrelated fields cannot satisfy coverage.
- Mixed acceptance dialects fail.
- Empty verifier configurations fail.
- Parsing and rendering are deterministic.

### Exit condition

One machine authority exists for scope and proof. No task needs synchronized copies of a command, path list, or expected result.

## Phase 3 — Executable expected results

Each check may declare expected exit, stdout/stderr contains/excludes/regex matchers, exact occurrence counts, and required/forbidden artifacts.

Gate must execute command once, preserve scrubbed output, evaluate every matcher, record structured results, and fail when any matcher fails. Invalid regexes, unreadable paths, spawn errors, and unsupported combinations fail.

### Tests

- Impossible output expectations fail after exit zero.
- Wrong exit, stdout, stderr, count, and artifact expectations fail independently.
- Invalid matcher definitions fail during lint.
- Evidence records are stable and complete.

### Exit condition

Every expected result is executable and recorded. Expected prose has no machine authority.

## Phase 4 — Strict progress

1. Replace copied mutable checklists with versioned progress events.
2. Validate events against known checklist leaf IDs.
3. Define allowed statuses and transitions.
4. Derive completed leaves, current leaf, and terminal state from events.
5. Reject duplicate headers/events, malformed or misplaced rows, unknown IDs/statuses, invalid transitions, and inconsistent derived state.
6. Keep handoff notes separate from state machine.

### Tests

- Every malformed grammar class fails.
- Invalid transitions fail.
- Header/event disagreement fails.
- Derived state is deterministic.
- Progress without gate evidence cannot authorize promotion.

### Exit condition

Progress has one unambiguous representation and cannot contradict itself.

## Phase 5 — Diagnostics and authoring UX

1. Give every finding stable rule ID, severity, package path, line, column, and message.
2. Label every package in batch text output.
3. Add stable JSON output.
4. Point findings at exact source fields or Markdown lines.
5. Remove arbitrary leaf-count and single-child restrictions.
6. Keep only rules tied to correctness, consistency, or research question.
7. Update template and author documentation for one current format.

### Tests

- Text and JSON snapshots include exact locations.
- Batch output distinguishes packages.
- Templates lint with zero warnings.

### Exit condition

Authors can locate and fix every finding directly. No advisory rule lacks defined purpose.

## Phase 6 — Corpus conversion

1. Convert TASK-001 through TASK-007 directly to new schemas.
2. Remove repeated commands, path lists, expected prose, generic protocol text, and clean-tree preconditions.
3. Remove future or inactive decision clauses from each task.
4. Fix remaining `seed.sql` decision/oracle inconsistencies.
5. Reduce documents by removing duplication, not task-specific behavior.
6. Separate positive requirements from prohibitions.
7. Define predecessor task and tree identity explicitly.
8. Treat TASK-007 as release-completion slice only when verifier proves one coherent outcome; otherwise split it.
9. Prove each baseline fails for documented reason.
10. Prove each trusted reference passes.

### Tests

- Every package lints with zero errors and warnings.
- Every baseline is red for stated reason.
- Every reference is green.
- Requirement-to-check coverage is complete.
- No package duplicates machine authority.

### Exit condition

Complete corpus is consistent, concise, executable, and warning-free.

## Phase 7 — Lifecycle and study separation

### Lifecycle

- Sequential tasks build on exact promoted predecessor.
- Each run records base, candidate tree, checks, evidence, result commit, and remote state.
- Resume uses recorded state and never silently changes repository or base.

### Study

Study schema contains case ID, invariant outcome/verifier, variants, repeats, blocks, random seed, primary endpoint, exclusions, and artifact policy.

Each observation records assignment, repeat, package/base/tree/verifier/harness/image/agent/model identities, normalized claim, gate result, duration, retries, rework, scope violations, and diff metrics.

Network remains available. Every observation starts from same immutable base and verifier. Study observations never promote into lifecycle state.

### Tests

- Assignment is deterministic and balanced.
- Same-base and same-verifier invariants are enforced.
- Failure cannot silently censor unrelated observations.
- Observation records are complete.
- Disabled end-to-end tests report `SKIP`, never `PASS`.
- Release verification proves enabled end-to-end bodies executed.

### Exit condition

Lifecycle reliability and format research use separate commands, schemas, records, and claims.

## Phase 8 — Documentation cleanup

Update root README, harness README, experiment README, CLI help, templates, and examples. Remove obsolete hostile-workspace, nested-isolation, disabled-lifecycle, legacy-reader, migration, old-schema, duplicated-authority, and lifecycle-as-study claims.

Documentation must describe only final current product.

## Final verification

Run in order:

1. `cargo fmt --manifest-path harness/Cargo.toml --check`
2. `cargo clippy --manifest-path harness/Cargo.toml --all-targets -- -D warnings`
3. `cargo test --manifest-path harness/Cargo.toml`
4. `cargo run --quiet --manifest-path harness/Cargo.toml -- selftest`
5. Source-built `taskfmt lint` across all packages with zero errors and warnings.
6. Exact-tree mutation and concurrent-main lease suites.
7. Matcher, coverage, strict-progress, and diagnostics JSON suites.
8. Lifecycle and study suites.
9. Enabled network/container end-to-end tests with proof bodies executed.
10. Baseline-red and reference-green verification for every task.
11. One disposable complete lifecycle run through promotion.
12. Proof remote `main` tree equals recorded gated tree.
13. `git diff --check`.
14. Clean worktree with local `main` equal to `origin/main`.

## Delivery rules

- Inspect and reproduce before editing.
- Fix responsible architecture, not only symptoms.
- Keep each slice runnable.
- Add focused regression proof with each change.
- Run focused tests before full suite.
- Commit only green coherent slices.
- Use `git commit -s` and include `Co-authored-by: Codex <codex@openai.com>`.
- Push commits directly to `main`.
- Do not declare completion until every final check has authoritative evidence.
