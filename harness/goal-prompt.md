# Launch prompt documentation

The runtime prompt lives only in [`src/task-prompt.md`](src/task-prompt.md). The CLI embeds that
file into the harness binary at compile time; it does not read this documentation file or a prompt
path from `experiment.toml` at runtime. Keep the shared `text claude codex` block under 4,000
characters; the evaluator (Haiku Stop hook) sees only the transcript, so the agent must surface
the verifier output. Everything else lives in `/task/AGENTS.md`.

`taskfmt-host run` selects the first fenced block in the embedded file whose info string names the
profile's agent kind (`text claude`, `text codex`, or shared `text claude codex`) and collapses it
to one line in `<run>/prompt.txt`. To diverge the two prompts, split the block into `text claude`
and `text codex`.

## Claude Code (`/goal`)

Headed (the harness): `taskfmt-host run --agent <claude profile>` starts `claude --dangerously-skip-permissions --session-id <uuid> --add-dir /task --add-dir /progress --model <M> --effort <E>` (`ops/container.rs claude_agent_cmd`) under herdr and sends the embedded task prompt with `herdr agent prompt task`; acceptance = `goal_status` sentinel in the session transcript (`run.rs confirm_acceptance`; `taskfmt-host status` reads the verdicts).

Headless (reference only; runs are headed under herdr — see `README.md`):

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

Dispatch is the headed Codex TUI, not `codex exec`: `taskfmt-host run --agent codex-default` launches `codex --dangerously-bypass-approvals-and-sandbox --no-alt-screen -C /work --add-dir /task --add-dir /progress [-m M] -c model_reasoning_effort="E"` (`ops/container.rs codex_agent_cmd`; `-m` only when the profile pins a model — `codex-default` pins none, effort `high`) under herdr and sends the embedded task prompt via `herdr agent prompt task` (`run.rs dispatch_one`). Codex Goals exist in the TUI since 0.128.0 (image pins 0.153.4); the goal text follows the cookbook shape (outcome, verification surface, constraints, boundaries, blocked stop condition) [O1]. Config pre-seed: `preseed_agent_home` writes `$CODEX_HOME/config.toml` (`container.rs codex_config_toml`) with `approval_policy = "never"`, `sandbox_mode = "danger-full-access"`, `[features] goals = false`, `/work` trusted. Native goals stay off so the Codex "Goal achieved" banner cannot be mistaken for harness completion; the codex prompt block is plain task text (no `/goal`) and requires a final `GOAL_RESULT task=<id> status=DONE` line.

Goal lifecycle commands documented for the Codex TUI [O1]: `/goal` (show), `/goal pause`, `/goal resume`, `/goal clear` (`taskfmt-host status --kill-after` sends the last one). A goal ends on success, pause, clear, interruption, budget limit, or a blocker needing user input; completion is evidence-based (files, tests, logs — not reasoning alone).

`run.rs confirm_acceptance` confirms only that herdr reports the agent `working`. `taskfmt-host status` reads `$CODEX_HOME/sessions/**/rollout-*.jsonl` under the run's `agent-home` bind mount as the authoritative `GOAL_RESULT` source (mirroring Claude's session jsonl), falls back to `tui.log`, reports `goal_verdicts: null`, and sets `native_goal_only: true` when the TUI shows "Goal achieved" without an anchored `GOAL_RESULT`. Current-pane/TUI activity can veto a false `idle`; `idle` or `done` alone is never completion evidence.

Headless `codex exec` has no goal flag (not first-class; findings §2 [O1]) — not used by the harness.
