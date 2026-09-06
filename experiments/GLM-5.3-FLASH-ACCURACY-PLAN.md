# GLM-5.3-Flash Task-Format Accuracy Campaign

Launch the campaign with `taskfmt experiment --proof-corpus /path/to/pgtui-proof.git`; it runs the
preflight before creating the runtime repository. The per-task agent prompt has one source at
`harness/src/task-prompt.md`, embedded by the CLI; do not copy it into this plan. The prompt
below is only the separate campaign-controller `/goal` prompt.

```text
/goal Run a controlled task-format accuracy campaign using profile zai-flash
(model glm-5.3-flash).

Preconditions:
1. Abort if corpus-preflight.sh fails.
2. Record taskfmt version, harness fingerprint, image fingerprints,
   Claude version, model, image digest, repo commit, and runtime settings.
3. Treat trusted gate results and workspace scope fingerprints as truth.
   GOAL_RESULT and model claims are secondary evidence.
4. Separate infrastructure failures from model, protocol, format, verifier,
   and harness failures.

Execution:
1. Run TASK-001 as lifecycle smoke only.
2. Run TASK-002 and TASK-003 as the pilot format stratum.
3. Run TASK-004 through TASK-006 as integration/infra stratum.
4. Run TASK-007 separately as artifact/PTY stratum.
5. Use a fresh repo clone and fresh task run for every trial.
6. Preserve manifest, prompt, progress log, Claude transcript, TUI log,
   gate log, gate evidence, fingerprints, final diff, and promotion record.

After every task:
1. Spawn four reviewers:
   - task-format contract/parser reviewer
   - trusted-verifier/ground-truth reviewer
   - harness/transcript/protocol reviewer
   - Docker/Postgres/model-infrastructure reviewer
2. Every finding must include severity, path:line, reproduction evidence,
   root enabling condition, and proposed fix.
3. Classify the result:
   INFRA, AGENT, PROTOCOL, FORMAT, VERIFIER, or HARNESS.
4. Never weaken a trusted gate to make a task pass.

Format experiments:
1. Compare baseline and candidate packages with the same task, model, image,
   repo, trusted overlay, verifier, and runtime.
2. Change one format variable at a time.
3. Repeat each paired trial at least 3 times.
4. Compare pass rate, false completion, scope violations, retries, turns,
   time, rework, diff churn, verifier disagreement, and infra-failure rate.
5. Do not infer format quality by comparing unrelated tasks.

Fix loop:
1. Reproduce the failure from the saved artifact bundle.
2. Fix the narrowest structural cause.
3. Add a regression test.
4. Run focused tests, full Rust tests, task lint, and selftest.
5. Rebuild images and verify host/image fingerprints match.
6. Rerun the failed task from a fresh clone.
7. Only then update the task template, goal prompt, or harness.
8. Rerun the affected stratum and final all-task campaign.

Done only when:
1. Corpus preflight passes.
2. All seven task packages lint and selfcheck.
3. Every final run has trusted gate PASS, valid scope fingerprint,
   complete evidence, and valid promotion.
4. No false completion, scope violation, or verifier bypass remains.
5. Any format change has repeated paired evidence showing equal or better
   accuracy without weakening verification.
6. Final artifacts are reproducible and the repository is clean on main.
```
