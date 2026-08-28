# Launch prompt

Keep the `/goal` condition under 4,000 characters; the evaluator (Haiku Stop hook) sees only the transcript, so the agent must surface the verifier output. Everything else lives in `/task/AGENT.md`.

## Claude Code (`/goal`)

```text
/goal Implement the single task in /task/README.md following /task/AGENT.md exactly.
Done when: /task/verify.sh has been run from /work in this session, exited 0, and
printed DONE as its last line, with that full output shown in the transcript;
/progress/progress.md has STATE: DONE with every checklist item [x]; and the final
report ending in a GOAL_RESULT line has been printed. Nothing under /task/ and no
protected_paths file may change. Do not weaken, skip, or delete checks. If a
precondition fails, stop with STATUS: BLOCKED; if the task needs a scope, decision,
or checklist change, stop with STATUS: NEEDS_REPLAN. Stop after 40 turns.
```

Headless (reference only; runs are headed under herdr — see `docs/research/notes/headed-herdr-harness.md`):

```sh
claude -p "/goal <condition above>" \
  --output-format stream-json --verbose \
  --dangerously-skip-permissions \
  --max-turns 40 --max-budget-usd 10 \
  --session-id "$SESSION_ID" \
  --add-dir /task --add-dir /progress
```

Container must run as a non-root user (the flag is rejected as root). Do not pass `--bare` (it skips hooks; `/goal` is a hook — UNVERIFIED that it survives `--bare`).

## Codex (`codex exec`)

Goals are not first-class in `codex exec` (no `--goal` flag as of 0.150.1). One exec = one attempt; retry with `codex exec resume --last "Continue until /task/verify.sh prints DONE"` up to N times from the harness.

```sh
codex exec --json --skip-git-repo-check -C /work \
  --dangerously-bypass-approvals-and-sandbox \
  --add-dir /task --add-dir /progress \
  -o /out/last-message.txt \
  "Implement the single task in /task/README.md following /task/AGENT.md exactly. Finish when /task/verify.sh exits 0 with last line DONE, /progress/progress.md has STATE: DONE, and the final report ending in GOAL_RESULT is printed. Nothing under /task/ may change. If a precondition fails stop with STATUS: BLOCKED; if scope or decisions must change stop with STATUS: NEEDS_REPLAN."
```
