# Launch prompt

Keep the `/goal` condition under 4,000 characters; the evaluator (Haiku Stop hook) sees only the transcript, so the agent must surface the verifier output. Everything else lives in `/task/AGENTS.md`.

## Block selection (`taskfmt run`)

`taskfmt run` injects exactly one fenced block from this file: the first ```` ```text ```` block whose info string names the profile's agent kind (`text claude`, `text codex`, or a shared `text claude codex`), collapsed to one line (`<run>/prompt.txt`). Blocks with any other fence (` ```sh `, plain ` ```text `) are never injected. To diverge the two prompts, split the shared block into `text claude` and `text codex`.

## Goal condition (shared: Claude Code `/goal`, Codex `/goal`)

```text claude codex
/goal Implement the single task in /task/README.md following /task/AGENTS.md exactly.
Done when: taskfmt verify has been run from /work after the last file change, exited 0, and
printed DONE as its last line, with that full output shown in the transcript;
/progress/progress.md has STATE: DONE with every checklist item [x]; and the final
report ending in a GOAL_RESULT line has been printed. Nothing under /task/ may change
and no file outside the expected_paths whitelist of /task/README.md may change. Do not
weaken, skip, or delete checks. If a
precondition fails, stop with STATUS: BLOCKED; if the task needs a scope, decision,
or checklist change, stop with STATUS: NEEDS_REPLAN. Stop after 40 turns; if stopping at the
cap, leave STATE: IN_PROGRESS and print STATUS: INCOMPLETE.
```

## Claude Code (`/goal`)

Headed (the harness): `taskfmt run --agent <claude profile>` starts `claude --dangerously-skip-permissions --session-id <uuid> --add-dir /task --add-dir /progress --model <M> --effort <E>` (`ops/container.rs claude_agent_cmd`) under herdr and sends the block above with `herdr agent prompt task`; acceptance = `goal_status` sentinel in the session transcript (`run.rs confirm_acceptance`; `taskfmt status` reads the verdicts).

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

## Codex (headed TUI, native `/goal`)

Dispatch is the headed Codex TUI, not `codex exec`: `taskfmt run --agent codex-default` launches `codex --dangerously-bypass-approvals-and-sandbox --no-alt-screen -C /work --add-dir /task --add-dir /progress [-m M] -c model_reasoning_effort="E"` (`ops/container.rs codex_agent_cmd`; `-m` only when the profile pins a model — `codex-default` pins none, effort `high`) under herdr and sends the same block above via `herdr agent prompt task` (`run.rs dispatch_one`). Codex Goals exist in the TUI since 0.128.0 (image pins 0.150.1); the goal text follows the cookbook shape (outcome, verification surface, constraints, boundaries, blocked stop condition) [O1]. Config pre-seed: `preseed_agent_home` writes `$CODEX_HOME/config.toml` (`container.rs codex_config_toml`) with `approval_policy = "never"`, `sandbox_mode = "danger-full-access"`, `[features] goals = true`, `/work` trusted — whether the `goals` key is still required at 0.150.1 is UNVERIFIED (the cookbook names no feature flag).

Goal lifecycle commands documented for the Codex TUI [O1]: `/goal` (show), `/goal pause`, `/goal resume`, `/goal clear` (`taskfmt status --kill-after` sends the last one). A goal ends on success, pause, clear, interruption, budget limit, or a blocker needing user input; completion is evidence-based (files, tests, logs — not reasoning alone).

UNVERIFIED until a run exists: that `/goal <condition>` submitted through `herdr agent prompt` sets a goal in the Codex TUI (slash popup vs Enter); that any goal event lands in `$CODEX_HOME/sessions/**/rollout-*.jsonl`. `run.rs confirm_acceptance` therefore confirms only that herdr reports the agent `working`; `taskfmt status` reports `goal_verdicts: null` and `transcript: n/a (rollout jsonl not parsed)` for Codex and relies on the `GOAL_RESULT` line in `/out/tui.log` plus herdr `idle`/`done`.

Headless `codex exec` has no goal flag (not first-class; findings §2 [O1]) — not used by the harness.
