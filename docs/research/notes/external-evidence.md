# External evidence: structuring task specs for autonomous coding agents

Scope: evidence gathered 2026-08-28 via web fetch/search, for the `task.md` + `verify.sh` + `progress.md` package defined in `docs/research/raw/01-initial-goal-template.md` (v1) and `docs/research/raw/02-checklist-checkboxes.md` (v2). Claims are paraphrased closely; direct quotes in quotation marks. Anything not read from the primary page is marked UNVERIFIED.

Conventions: "v2 template" = `GOAL-TASK-TEMPLATE.md` schema `goal-task/v2` in message 02.

---

## 1. Claude Code official docs

### 1.1 `/goal` — https://code.claude.com/docs/en/goal (fetched 2026-08-28; page mentions v2.1.246)

- Mechanism: "`/goal` is a wrapper around a session-scoped prompt-based Stop hook. Each time Claude finishes a turn, Claude Code sends the condition and the conversation so far to your configured small fast model, which defaults to Haiku." Three verdicts: **Not yet met** (reason fed back as guidance), **Met** (goal cleared), **Impossible** (goal cleared, failed entry recorded).
- What the evaluator sees: "The evaluator judges your condition against what Claude has surfaced in the conversation. It doesn't run commands or read files independently, so write the condition as something Claude's own output can demonstrate." And: "It does not call tools, so it can only judge what Claude has already surfaced in the conversation."
- Effective condition has: "One measurable end state", "A stated check: how Claude should prove it, such as `npm test` exits 0", "Constraints that matter: anything that must not change on the way there, such as 'no other test file is modified'". Condition max **4,000 characters**.
- Bounding: "include a turn or time clause in the condition, such as `or stop after 20 turns`."
- Stall protection: "If Claude keeps answering the evaluator without making progress (no tool use for several turns in a row), Claude Code stops the loop, prints a warning, and returns control to you with the goal still set."
- Errors that clear the goal: auth failure, exhausted credits, "A context overflow that auto-compaction couldn't clear", model unavailable. Rate limits/overloads leave goal active.
- Background work: evaluation skipped while subagent/background shell running; check-in after 30 min (`CLAUDE_CODE_GOAL_CHECKIN_MINUTES`, `0` disables).
- Non-interactive: `claude -p "/goal <condition>"`; add `--output-format stream-json --verbose` to see progress. Setting a goal "starts a turn immediately, with the condition itself as the directive."
- Permissions: "A goal doesn't change your permission mode." Unattended runs need auto mode or allowlists.
- Resume: goal restored on `--continue`/`--resume`; turn count/timer reset.
- Evaluator model override: `ANTHROPIC_DEFAULT_HAIKU_MODEL`. Requires hooks not disabled (`disableAllHooks`, `allowManagedHooksOnly`).

### 1.2 Best practices — https://code.claude.com/docs/en/best-practices (fetched 2026-08-28)

- Core constraint: "Claude's context window fills up fast, and performance degrades as it fills."
- "Give Claude a way to verify its work": "Claude stops when the work looks done. Without a check it can run, 'looks done' is the only signal available." Ladder of gating: in one prompt → `/goal` → "As a deterministic gate: a Stop hook runs your check as a script and blocks the turn from ending until it passes. Claude Code overrides the hook and ends the turn after 8 consecutive blocks." → "By a second opinion: a verification subagent ... so the agent doing the work isn't the one grading it."
- "Have Claude show evidence rather than asserting success: the test output, the command it ran and what it returned, or a screenshot of the result."
- Specificity examples: "write a failing test that reproduces the issue, then fix it"; "address the root cause, don't suppress the error".
- Spec guidance: "The most useful specs are self-contained: they name the files and interfaces involved, state what is out of scope, and end with an end-to-end verification step that proves the feature works." "Once the spec is complete, start a fresh session to execute it."
- Planning: "If you could describe the diff in one sentence, skip the plan."
- CLAUDE.md: "keep it short and human-readable"; "For each line, ask: 'Would removing this cause Claude to make mistakes?' If not, cut it. Bloated CLAUDE.md files cause Claude to ignore your actual instructions!" "If you emphasize many lines, none of them stands out."
- Hooks: "Unlike CLAUDE.md instructions which are advisory, hooks are deterministic and guarantee the action happens."
- Adversarial review: "A reviewer running in a fresh subagent context sees only the diff and the criteria you give it". Caveat: "A reviewer prompted to find gaps will usually report some, even when the work is sound ... Tell the reviewer to flag only gaps that affect correctness or the stated requirements."
- Fan-out: `claude -p "..." --allowedTools "Edit,Bash(git commit *)"`; "`--allowedTools` flag restricts what Claude can do, which matters when you're running unattended."
- Failure patterns: "The trust-then-verify gap ... If you can't verify it, don't ship it." "After two failed corrections, `/clear` and write a better initial prompt."
- Compaction: "Customize compaction behavior in CLAUDE.md with instructions like 'When compacting, always preserve the full list of modified files and any test commands'."

### 1.3 CLAUDE.md / memory — https://code.claude.com/docs/en/memory (fetched 2026-08-28)

- "Claude treats them as context, not enforced configuration. To block an action regardless of what Claude decides, use a PreToolUse hook instead."
- Size: "target under 200 lines per CLAUDE.md file. Longer files consume more context and reduce adherence." Files >4 MiB skipped.
- Structure/specificity: "use markdown headers and bullets"; "write instructions that are concrete enough to verify"; "if two rules contradict each other, Claude may pick one arbitrarily."
- Delivery: "CLAUDE.md content is delivered as a user message after the system prompt, not as part of the system prompt itself."
- HTML comments: "Block-level HTML comments (`<!-- maintainer notes -->`) in CLAUDE.md files are stripped before the content is injected into Claude's context." (Applies to CLAUDE.md load path; a task.md read via the Read tool keeps comments: "When you open a CLAUDE.md file directly with the Read tool, comments remain visible.")
- Compaction survival: "Project-root CLAUDE.md survives compaction: after `/compact`, Claude re-reads it from disk." Conversation-only instructions do not.
- `.claude/rules/*.md` with `paths:` frontmatter for path-scoped rules. `@path` imports (depth 4). `AGENTS.md` not read natively; use `@AGENTS.md` import or symlink.

### 1.4 Hooks — https://code.claude.com/docs/en/hooks and https://code.claude.com/docs/en/hooks-guide (fetched 2026-08-28; reference also downloaded as markdown)

Event names (exact): `SessionStart`, `SessionEnd`, `Setup`, `UserPromptSubmit`, `UserPromptExpansion`, `Stop`, `StopFailure`, `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `PostToolBatch`, `PermissionRequest`, `PermissionDenied`, `SubagentStart`, `SubagentStop`, `TaskCreated`, `TaskCompleted`, `TeammateIdle`, `FileChanged`, `CwdChanged`, `DirectoryAdded`, `ConfigChange`, `InstructionsLoaded`, `PreCompact`, `PostCompact`, `Notification`, `MessageDisplay`, `WorktreeCreate`, `WorktreeRemove`, `Elicitation`, `ElicitationResult`.

- Exit codes: `0` success (stdout parsed), `2` blocking error, other = non-blocking. Events that block on exit 2 include `PreToolUse`, `UserPromptSubmit`, `Stop`, `SubagentStop`, `TaskCompleted`, `PreCompact`, `PostToolBatch`. **`PostToolUse` cannot block** ("output ignored on exit 2") — it can only feed `decision: "block"` + `reason` text back to Claude as feedback / `additionalContext`.
- Stop decision control (verbatim from reference): top-level `decision`: "`\"block\"` prevents Claude from stopping. Omit to allow Claude to stop"; `reason`: "Required when `decision` is `\"block\"`"; `hookSpecificOutput.additionalContext`: "Non-error feedback for Claude." "A hook that blocks by exiting 2 routes the same way as `reason`." PreToolUse uses `hookSpecificOutput.permissionDecision` (`allow|deny|ask|defer`) + `permissionDecisionReason`; "Other events like PostToolUse and Stop continue to use top-level `decision` and `reason`."
- Stop input fields: `stop_hook_active`, `last_assistant_message`, `background_tasks`, `session_crons`. "The `stop_hook_active` field is `true` when Claude Code is already continuing as a result of a stop hook. Check this value ... to avoid blocking on a condition that will never resolve. Claude Code overrides the hook and ends the turn after 8 consecutive blocks." Cap env: `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`.
- Prompt-based hooks: `"type": "prompt"`, `prompt`, optional `model`, `timeout` (default 30s). Model returns `{"ok": true|false, "reason": ...}`; for Stop, "`\"ok\": false` ... the `reason` is fed back to Claude so it keeps working, unless the response also sets `\"impossible\": true`" which allows the stop. Agent-based hooks (`type: "agent"`) also exist (can use tools) — UNVERIFIED details, not read.
- PreToolUse policy enforcement: "A hook that returns `permissionDecision: \"deny\"` blocks the tool even in `bypassPermissions` mode or with `--dangerously-skip-permissions`." Hooks can tighten, never loosen, permissions. Bash-command matcher filtering "fails open ... use the permission system rather than a hook to enforce a hard allow or deny."
- Protected files example (hooks-guide): `PreToolUse` matcher `Edit|Write`, script checks `tool_input.file_path` against `PROTECTED_PATTERNS` and `exit 2` with message on stderr.
- Re-inject context after compaction: `SessionStart` with `"matcher": "compact"`; stdout is added to context. Other `SessionStart` matchers: `startup`, `resume`, `clear`, `compact`, `fork`.
- Multiple hooks: all run; most restrictive `PreToolUse` decision wins (`deny` > `defer` > `ask` > `allow`).
- Command hook example that blocks stop until `npm test` passes exists in the reference (`.claude/hooks/block-stop-until-tests.sh`, configured via `"Stop": [{"hooks":[{"type":"command","command":"${CLAUDE_PROJECT_DIR}/.claude/hooks/block-stop-until-tests.sh","timeout":60}]}]`).

### 1.5 Headless / `claude -p` — https://code.claude.com/docs/en/headless and https://code.claude.com/docs/en/cli-reference (fetched 2026-08-28)

Exact flags: `-p`/`--print`; `--allowedTools` (permission-rule syntax, e.g. `"Bash(git commit *)"`; "The space before `*` is important"); `--disallowedTools`; `--tools`; `--permission-mode {default|acceptEdits|plan|auto|dontAsk|bypassPermissions|manual}`; `--dangerously-skip-permissions` ("equivalent to `--permission-mode bypassPermissions`"; CLI rejects it when run as root); `--max-turns` and `--max-budget-usd` (print mode only); `--output-format {text|json|stream-json}`; `--json-schema` (structured output in `structured_output` field); `--append-system-prompt`, `--append-system-prompt-file`, `--system-prompt`, `--system-prompt-file`; `--bare` ("recommended mode for scripted and SDK calls, and will become the default for `-p`"; skips hooks, skills, CLAUDE.md, MCP, auto memory — pass context via flags); `--settings`; `--add-dir`; `-w`/`--worktree`; `--continue`, `--resume <id>`, `--fork-session`; `--no-session-persistence`; `--verbose`; `--include-partial-messages`; `--forward-subagent-text`; `--mcp-config`; `--agents`.
- Exit status: "exits with code 0 on success and a non-zero code when the run fails". SIGTERM → exit 143, turn unfinished.
- `dontAsk` mode: "denies anything not in your `permissions.allow` rules or the read-only command set, which is useful for locked-down CI runs."
- Security: "Without `--bare`, a `-p` session runs the hooks in a project's `.claude/settings.json` and connects the servers in its `.mcp.json`, even in a folder you've never trusted."
- Skills/commands usable in `-p` by including `/skill-name` in the prompt string.

### 1.6 Dev container reference — https://code.claude.com/docs/en/devcontainer (fetched 2026-08-28)

- Files: `.devcontainer/devcontainer.json` (mounts, `runArgs` with `NET_ADMIN`/`NET_RAW`, `containerEnv`), `Dockerfile`, `init-firewall.sh` ("Blocks all outbound network traffic except the allowed domains"). Feature: `ghcr.io/anthropics/devcontainer-features/claude-code:1.0`.
- "Because the container runs Claude Code as a non-root user and confines command execution to the container, you can pass `--dangerously-skip-permissions` for unattended operation."
- Warning: "When executed with `--dangerously-skip-permissions`, dev containers do not prevent a malicious project from exfiltrating anything accessible inside the container, including the Claude Code credentials stored in `~/.claude`." Avoid mounting `~/.ssh`.
- Policy: `/etc/claude-code/managed-settings.json` highest precedence; `permissions.disableBypassPermissionsMode: "disable"` blocks the flag entirely. Persist auth via named volume at `~/.claude` + `CLAUDE_CONFIG_DIR`.

### 1.7 Anthropic engineering posts

**Effective harnesses for long-running agents** — https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents (2025-11-26)
- Failure: "Often, this led to the model running out of context in the middle of its implementation, leaving the next session to start with a feature half-implemented." "A later agent instance would look around, see that progress had been made, and declare the job done."
- Fix: "The next iteration of the coding agent was then asked to work on only one feature at a time." Feature list JSON, "all initially marked as 'failing'"; "It is unacceptable to remove or edit tests because this could lead to missing or buggy functionality."
- Session protocol: "Read the git logs and progress files to get up to speed"; "End the session by writing a git commit and progress update." Progress file `claude-progress.txt`.
- Verification: "Claude mostly did well at verifying features end-to-end once explicitly prompted to use browser automation tools." Unit tests alone were insufficient for visible behavior.

**Harness design for long-running application development** — https://www.anthropic.com/engineering/harness-design-long-running-apps (2026, exact date UNVERIFIED)
- Planner / Generator / Evaluator split. Self-evaluation bias: "agents tend to respond by confidently praising the work—even when, to a human observer, the quality is obviously mediocre." Tuning a standalone evaluator toward skepticism is "far more tractable" than making generators self-critical.
- "Sprint contracts: Generator and evaluator negotiate testable success criteria before implementation."
- Context: for Sonnet 4.5, full context resets beat compaction ("context anxiety"); with Opus 4.6 "the model could natively handle the job without this sort of decomposition" — decomposition need is model-dependent.

**Effective context engineering for AI agents** — https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents (2025)
- "Right altitude": avoid "brittle if-else hardcoded logic" and "vague, high-level guidance"; be "specific enough to guide behavior effectively, yet flexible enough to provide strong heuristics."
- "organizing prompts into distinct sections" with "XML tagging or Markdown headers".
- "the minimal set of information that fully outlines your expected behavior" — not brevity for its own sake.
- Long-horizon: compaction, "structured note-taking" (persistent memory outside context), sub-agents returning "condensed, distilled summaries".

**Building agents with the Claude Agent SDK** — https://claude.com/blog/building-agents-with-the-claude-agent-sdk (2025-09-29)
- Loop: "gather context -> take action -> verify work -> repeat." Verification strategies ranked: rules-based feedback ("providing clearly defined rules for an output, then explaining which rules failed and why" — linting as exemplar), visual feedback (screenshots), LLM-as-judge ("heavy latency tradeoffs").

**Demystifying evals for AI agents** — https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents (2025/2026, date UNVERIFIED)
- "A good task is one where two domain experts would independently reach the same pass/fail verdict."
- "Deterministic graders are natural for coding agents because software is generally straightforward to evaluate: does the code run and do the tests pass?"
- "For each task, it's useful to create a reference solution: a known working output that passes all graders. This proves that the task is solvable and verifies graders are correctly configured."
- Anti-pattern: "check that agents followed very specific steps like a sequence of tool calls in the right order. We've found this approach too rigid." Ambiguous specs (unstated file paths etc.) cause unfair failures.

---

## 2. OpenAI Codex

### 2.1 Goals cookbook — https://developers.openai.com/cookbook/examples/codex/using_goals_in_codex (2026, date UNVERIFIED)

- Goal = "a persistent objective in Codex that keeps a thread working toward a defined outcome across turns."
- Six elements of a strong goal: **Outcome**, **Verification surface** ("test, benchmark, artifact, or evidence that proves success"), **Constraints** (what must not regress), **Boundaries** ("allowed files, tools, resources, or repositories"), **Iteration policy** ("how to choose the next action after each attempt"), **Blocked stop condition** ("when to halt and report no defensible path remains").
- Pattern: `/goal [desired end state] verified by [specific evidence] while preserving [constraints]. Use [allowed inputs]. Between iterations, [decision logic]. If blocked, [report and unlock steps].`
- Completion "evidence-based": model compares objective to "concrete evidence (files changed, tests passed, benchmark output) rather than declaring success from reasoning alone." Continuation is "event-driven", checked at safe boundaries (turn end, idle, no queued input).
- Anti-patterns: one-line edits; "Vague finish lines ('improve performance')"; hiding uncertainty about data availability; "Tasks without auditable completion conditions".
- Commands: `/goal`, `/goal pause`, `/goal resume`, `/goal clear`.

### 2.2 Follow goals use-case — https://learn.chatgpt.com/use-cases/follow-goals (redirect from developers.openai.com/codex/use-cases/follow-goals)

- Goal size: "bigger than one prompt but smaller than an open-ended backlog"; avoid a "loose list of unrelated work".
- Specify "what Codex should achieve", "commands or artifacts that prove progress", "what it shouldn't change", and "one verifiable stopping condition".
- Iteration: "work in checkpoints and keep a short progress log"; status updates name "the current checkpoint, what was verified, what remains", plus blockers. "tighten the goal rather than adding more one-off instructions."
- Codex "will stop running when it's confident it has reached the stopping condition."

### 2.3 Execution plans cookbook — https://developers.openai.com/cookbook/articles/codex_exec_plans (2025-10)

- Required sections: "Purpose / Big Picture", "Progress" (checkbox list with timestamps), "Surprises & Discoveries", "Decision Log", "Outcomes & Retrospective". Recommended: Context, Plan of Work, Concrete Steps, Validation, Idempotence, Artifacts, Interfaces.
- "Every stopping point must be documented here, even if it requires splitting a partially completed task into two." Progress must reflect actual state; timestamps measure rate: `- [x] (2025-10-01 13:00Z) Completed step.`
- Living document: "revise it as progress is made, as discoveries occur, and as design decisions are finalized"; on revision "write a note at the bottom of the plan describing the change and the reason why." Cited example: seven hours of continuous model work.
- Note: this format is deliberately **mutable** by the executor (plan = working doc), the opposite of the immutable-contract stance in v2.

### 2.4 Harness engineering — https://openai.com/index/harness-engineering/ (2026-02/03; primary page returned 403; content read via mirrors https://xiaow.dev/... and https://zby.github.io/commonplace/... — quotes UNVERIFIED against original)

- "Humans steer. Agents execute."
- AGENTS.md "~100 lines" as "a map, not a manual"; "one big AGENTS.md" failed: "A giant instruction file crowds out the task, the code, and the relevant docs—so the agent either misses key constraints or starts optimizing for the wrong ones."
- "Anything Codex can't access in-context effectively doesn't exist." → invest in perception: Chrome DevTools Protocol, per-worktree logs/metrics, bootable app per worktree.
- Mechanical enforcement: custom linters with "remediating instructions embedded directly in lint error messages"; "enforce boundaries centrally, allow autonomy locally"; CI linters validate docs freshness and cross-links.
- "Garbage collection": recurring doc-gardening / drift-scan agents opening refactor PRs.
- Results: ~1M lines, ~1,500 PRs in ~10x less time (UNVERIFIED numbers).

### 2.5 Symphony — https://github.com/openai/symphony and SPEC.md (blog at openai.com returned 403)

- README: shift "from supervising coding agents to managing work that needs to get done." Agents provide "proof of work" (CI status, PR review feedback, complexity analysis, walkthrough videos); "agents land the PR safely."
- SPEC: `WORKFLOW.md` = YAML front matter (`tracker`, `polling`, `workspace`, `hooks`, `agent`, `codex`) + body rendered as Jinja-like prompt with `issue` and `attempt` vars; "Remaining lines become the prompt body." Issue states are tracker-native (`Todo`/`In Progress`/`Done`); config declares **active** vs **terminal** states. "Workspace path MUST remain under configured workspace root"; one deterministic workspace per issue. `blocked_by` is "best-effort provider metadata" — scheduler blocking left to adapters. Success "can end at a workflow-defined handoff state (for example `Human Review`), not necessarily `Done`." Hooks: `after_create`, `before_run`, `after_run`, `before_remove`; `max_concurrent_agents` default 10; exponential retry backoff.
- Takeaway: task content lives in the tracker issue; the harness supplies the fixed prompt body, workspace, and lifecycle. Issue is the contract; agent does not edit it.

### 2.6 AGENTS.md — https://agents.md/ and https://learn.chatgpt.com/docs/agent-configuration/agents-md

- "README for agents"; used by >60k projects. Contents: build/setup commands, testing instructions, code style, security, conventions. Nesting: "closest file to the code being edited takes precedence"; "Explicit user prompts override all file-based instructions."
- Codex chain: `~/.codex/AGENTS.override.md` else `~/.codex/AGENTS.md`; then from git root down to cwd, `AGENTS.override.md` then `AGENTS.md`; "Codex concatenates files from the root down." Cap `project_doc_max_bytes` = 32 KiB default. Advice: "Keep rules concise and actionable ... reserve lint checks for CI."

### 2.7 `codex exec` — https://learn.chatgpt.com/docs/non-interactive-mode

- `codex exec "<task-prompt>"`; progress to stderr, final message to stdout. Flags: `--sandbox {read-only|workspace-write|danger-full-access}` (default read-only), `--full-auto` (deprecated → `--sandbox workspace-write`), `--json` (JSONL events `thread.started`, `turn.started`, `turn.completed`, `item.completed`, `error`), `-o`/`--output-last-message <path>`, `--output-schema <path>`, `--ephemeral`, `--ignore-user-config`, `--ignore-rules`, `--skip-git-repo-check`, `--cd <dir>`. Resume: `codex exec resume --last "<follow-up>"` / `codex exec resume <SESSION_ID>`. Stdin: `cat prompt.txt | codex exec -`. `--ask-for-approval` not on this page (UNVERIFIED).

---

## 3. Eval harness task formats

### 3.1 SWE-bench — https://www.swebench.com/SWE-bench/guides/datasets/ ; https://huggingface.co/datasets/princeton-nlp/SWE-bench_Verified

Fields: `instance_id`, `repo`, `base_commit`, `problem_statement` ("The issue title and body"), `hints_text` ("Comments made on the issue prior to the creation of the solution PR's first commit"), `version`, `environment_setup_commit`, `created_at`, `FAIL_TO_PASS` ("tests resolved by the PR"), `PASS_TO_PASS` ("tests that should pass before and after"), `test_patch` (tests from solution PR), `patch` (gold solution, hidden), `difficulty` (Verified only).
- Agent sees: repo at `base_commit` + `problem_statement` (+ optionally hints). Hidden: `patch`, `test_patch`, FAIL_TO_PASS/PASS_TO_PASS names. Grader applies `test_patch` then runs both lists. Verified = 500 human-validated instances.

### 3.2 SWE-agent default prompt — https://raw.githubusercontent.com/SWE-agent/SWE-agent/main/config/default.yaml

- System: "You are a helpful assistant that can interact with a computer to solve tasks."
- Instance template wraps issue in `<pr_description>`; "Your task is to make the minimal changes to non-tests files in the {{working_dir}} directory to ensure the <pr_description> is satisfied." Five steps: find relevant code → "Create and run a reproduction script" → edit → rerun repro → "Consider edge cases". "you DON'T have to modify the testing logic or any of the tests in any way!" Submission review reverts test-file changes with `git checkout -- <test file>`. Env: `PAGER=cat`, `GIT_PAGER=cat`, `TQDM_DISABLE=1`.
- Problem statement types: `TextProblemStatement`, `GithubIssue`, `FileProblemStatement` (https://swe-agent.com/latest/reference/agent_config/).

### 3.3 Terminal-Bench 2.0 / Harbor — https://www.harborframework.com/docs/tasks ; https://www.harborframework.com/docs/tasks/task-tutorial ; https://arxiv.org/abs/2601.11868 (ICLR 2026)

- Layout: `instruction.md`, `task.toml` (`[task]`, `[environment]` incl. `network` policy, `[agent] timeout_sec`, `[verifier] timeout_sec` / `environment_mode = "separate"`, `[metadata]` difficulty/category/tags), `environment/Dockerfile`, `solution/solve.sh` (oracle), `tests/test.sh`.
- Hidden tests: "The tests directory is copied at runtime to `/tests` — agents cannot see test logic during execution."
- Verifier contract: "the test script must produce a reward file in the `/logs/verifier/` directory" — `reward.txt` single number (0/1) or `reward.json` metrics. Verifier can run in a separate container.
- Instruction style: describe the goal, not steps — e.g. "the service starts, but the generated report is wrong" rather than "go to line 39 and fix the setdefault call"; be explicit about output paths/formats.
- Paper QC: instructions "sufficiently detailed and self-contained—agents should succeed based solely on what's explicitly stated"; oracle must pass tests; "Adversarial Exploit Agent" hunts reward hacks; contributor checklist + check tools. 89 tasks; 32,155 trials.

### 3.4 "What Makes a Good Terminal-Agent Benchmark Task" — https://arxiv.org/abs/2604.28093 (Bercovich, 2026-04-30)

- "A prompt is designed to help the agent succeed; a benchmark is designed to find out if it can." (Explicitly distinguishes prompts from benchmark tasks — our template is a prompt, so "help succeed" applies.)
- Anti-patterns: AI-generated verbose instructions; over-prescriptive step-by-step ("Tell the agent...what the end state should be"); clerical difficulty; hidden knowledge assumed by oracle; "Tests should verify outcomes, not implementations"; reward-hackable environments ("over 15% of tasks in popular terminal-agent benchmarks are reward-hackable"). Instruction length: ~two paragraphs, "as if it's intended for a smart human".

### 3.5 Long-Horizon Terminal-Bench — https://arxiv.org/abs/2607.08964 (2026-07)

- 46 tasks, "fine-grained graded subtasks" giving "dense intermediate rewards and partial credit". Best model 15.2% pass@1 at ≥0.95 reward; ~9.9M tokens, ~85 min per task. Supports the case for decomposing long work into independently graded checkpoints.

---

## 4. Vendor task-writing guidance

### 4.1 GitHub Copilot coding agent — https://docs.github.com/en/copilot/using-github-copilot/coding-agent/best-practices-for-using-copilot-to-work-on-tasks

- Issue must have: "A clear description of the problem to be solved or the work required"; acceptance criteria ("such as whether unit tests are needed"); "Directions about which files need modification". "Think of your issue as an AI prompt."
- Suited: bug fixes, UI, test coverage, docs, accessibility, tech debt. Not suited: broad cross-repo refactors, production-critical/security-sensitive, ambiguous tasks. Custom instructions: `.github/copilot-instructions.md`. Batch review comments via "Start a review".

### 4.2 Devin — https://docs.devin.ai/essential-guidelines/instructing-devin-effectively

- Four elements: helpful context (repo + purpose), step-by-step instructions, "Clear Success Criteria", references to existing patterns. "be as specific as possible."
- "Avoid vague success criteria" like "Make sure it works" — instead "verify that it has at least 500 rows" / "confirm the endpoint returns status 200". Break into checkpoints; "report back after completing each checkpoint or sub-task."
- Poor performance when instructions "Leave significant design decisions to Devin without guidance" or "Lack defined success metrics or verification steps".

### 4.3 Amp — https://ampcode.com/docs/prompting

- "Instead of 'can you do X?', try 'do X.'" "Don't try to make the model guess. If you know ... which files to look at, which commands to run — put it in your prompt." "Use one thread per task." "Tell the agent how to best review its work: what command or test to run, what URL to open, which logs to read." Explicit boundaries ("Do not edit any files").

### 4.4 Cursor cloud agents — https://cursor.com/docs/background-agent

- Environment via `.cursor/environment.json`; hooks via `.cursor/hooks.json`; agents "work on a separate branch, then push changes"; "produce merge-ready PRs with artifacts to demo their changes" (screenshots, videos, logs). No prompt-structure guidance on the page.

### 4.5 Community/advisory (weaker evidence)

- Addy Osmani, "How to write a good spec for AI agents" — https://addyosmani.com/blog/good-spec/ (analysis of "over 2,500 agent configuration files", no quantified outcomes): put "full commands with flags" early; "One real code snippet ... beats three paragraphs"; three-tier boundaries (always / ask first / never); "Never commit secrets" most common useful constraint.
- O'Reilly Radar / Stack Overflow, "The right amount of spec for agentic development" (2026-08-21) — https://www.oreilly.com/radar/the-right-amount-of-spec-for-agentic-development/ : "Cites no formal studies." For single bounded tasks: "structured intent: the goal, a few examples, nongoals, and clear acceptance criteria"; deterministic work warrants "BDD, contract tests, executable acceptance criteria"; multi-agent pipelines need "typed contracts + validators". Over-spec risk for exploratory work.
- Spec-driven development tooling (GitHub Spec Kit 2025-09, AWS Kiro 2025-07, EARS-notation acceptance criteria) — marketing-grade sources; no controlled comparison of structured vs prose found. **No peer-reviewed study comparing task-spec formats on agent outcomes was located (UNVERIFIED that none exists).**

### 4.6 GFM task lists — https://github.github.com/gfm/ §5.3 (via mirror snippets)

- Marker = `[` + (whitespace | `x` | `X`) + `]`; "If the character between the brackets is a whitespace character, the checkbox is unchecked. Otherwise, the checkbox is checked." Only two states; nesting follows ordinary list-item rules.

---

## Implications for our template (v2)

### Gets right (with citing source)

1. **Verifier-runnable, evidence-in-transcript completion** (`./verify.sh` exit 0 + `DONE` shown in transcript). Matches `/goal` docs: evaluator "doesn't run commands or read files independently" and best-practices "show evidence rather than asserting success". Matches Codex "evidence-based" completion.
2. **Stating the task ID / goal / AC IDs early and the verifier result at the end** — directly required by `/goal` evaluator visibility model (§1.1) and Codex "compact" checkpoint reporting (§2.2).
3. **One outcome per task, fresh context per task** — Anthropic harness post ("only one feature at a time", context exhaustion mid-feature) and Codex goal sizing ("bigger than one prompt but smaller than an open-ended backlog").
4. **Explicit constraints / out-of-scope / protected paths** — `/goal` docs list "Constraints that matter: anything that must not change" as one of three condition components; Codex goal elements include Constraints + Boundaries; SWE-agent hard-codes "don't modify tests"; Anthropic feature-list prompt: "unacceptable to remove or edit tests".
5. **Separate progress log + read progress/git log on resume** — Anthropic `claude-progress.txt` + "Read the git logs and progress files"; Codex exec-plans "Every stopping point must be documented".
6. **Fresh reviewer (Layer 3) separate from executor** — Anthropic harness-design ("self-evaluation bias"), best-practices adversarial review subagent.
7. **Fixed decisions, executor discretion; no unresolved product/API decisions** — Devin: poor results when instructions "leave significant design decisions to Devin without guidance"; best-practices "separate research and planning from implementation".
8. **Baseline/reproduction before edit** — SWE-agent default prompt step 2 ("Create and run a reproduction script"); best-practices "write a failing test that reproduces the issue, then fix it".
9. **Observable end-state ACs, tests verify outcomes not implementation** — Terminal-Bench/Harbor "describe the goal, not the steps"; Bercovich "Tests should verify outcomes, not implementations"; Anthropic evals "two domain experts would independently reach the same pass/fail verdict".
10. **Binary checkbox semantics only (`[ ]`/`[x]`), no `[-]`/`[~]`** — GFM spec allows only whitespace vs non-whitespace; v2's "current leaf" indirection is the correct way to get three states.
11. **Hidden/protected verifier as deterministic gate, not prose** — Harbor copies `tests/` at runtime so agents cannot see them; best-practices "hooks are deterministic ... CLAUDE.md instructions are advisory"; memory docs "use a PreToolUse hook instead".

### Gets wrong / risks

1. **Length and duplication.** v2 task.md is several hundred lines of boilerplate per task (semantics, protocol, prohibited shortcuts, completion report, author validation). Every source on instruction files warns that bulk reduces adherence: CLAUDE.md "target under 200 lines", "Bloated CLAUDE.md files cause Claude to ignore your actual instructions", "If you emphasize many lines, none of them stands out"; OpenAI "A giant instruction file crowds out the task, the code, and the relevant docs"; Bercovich ~two paragraphs "as if for a smart human"; context-engineering "minimal set of information that fully outlines your expected behavior". **Fix:** move invariant protocol (checklist semantics, prohibited shortcuts, report format, blocked/replan policy, execution protocol) into one repo-level file (`.agent/GOAL-TASK-PROTOCOL.md`, loaded via `@` import in CLAUDE.md or `--append-system-prompt-file`), leave task.md with only task-specific content. The author-validation HTML comment is stripped only on the CLAUDE.md path; in a task.md read with the Read tool it costs tokens — remove it from instances (v1 already says so; enforce with a linter).
2. **`/goal` invocation exceeds the 4,000-character condition limit risk.** The v2 invocation prose is long; combined with `@task.md` expansion it may hit the cap (`/goal` docs: "The condition can be up to 4,000 characters"). Keep the condition short (end state + check + constraints + turn bound) and reference task.md for the rest.
3. **No turn/time bound in the condition.** `/goal` docs recommend "or stop after 20 turns"; `-p` supports `--max-turns` and `--max-budget-usd`. v2 invocation has neither.
4. **Mutable regions inside task.md weaken the "immutable contract" and complicate hash gating.** v2 acknowledges this and requires normalization. Evidence from harnesses keeps contract and state in different artifacts: SWE-bench/Harbor instruction is read-only, Symphony issue is in the tracker, Anthropic uses a separate feature-list JSON + progress file, Codex exec-plans keep the whole plan mutable. Either direction is externally supported; the hybrid (partially mutable contract) is not seen in any source. Prefer: checklist + status in `progress.md` (or a JSON state file), task.md fully read-only; enforce with a `PreToolUse` hook (`Edit|Write` matcher, exit 2) rather than prose.
5. **`PostToolUse` cannot gate.** v1 §"Claude Code hardening" implies post-tool gates; hooks reference: PostToolUse "cannot block (output ignored on exit 2)". Gates must be `PreToolUse` (deny) or `Stop` (block with `decision: "block"`). Also `Stop` hooks are overridden after 8 consecutive blocks (`CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`) — the outer orchestrator, not the hook, must be the final authority (v1 Layer 2 already says this; make it explicit that in-session gates are best-effort).
6. **Per-turn `GOAL_PROGRESS` line + leaf-percentage bookkeeping is unsupported by evidence and adds overhead.** Codex asks for "compact" checkpoint status (current checkpoint, verified, remaining, blockers); nothing external asks for percentage roll-ups, leaf-only counting, or 4-level decimal IDs. Anthropic's feature list is a flat JSON with `passes: true/false`. Recommend a flat or 2-level checklist with stable IDs and a 3–4 field status line; drop percentage math and "roll up parents" rules (each rule is another instruction competing for attention).
7. **Checklist ordering rule "numeric depth-first, do not skip an earlier leaf" is over-prescriptive.** Bercovich and Harbor: describe end state, not steps; Anthropic evals: grading "specific steps ... in the right order" is "too rigid". Order should be a dependency hint, not a MUST.
8. **Weak reward hygiene for verify.sh.** Sources require an oracle: Harbor `solution/solve.sh` must pass `tests/test.sh`; Anthropic evals: "create a reference solution ... proves that the task is solvable and verifies graders are correctly configured"; Terminal-Bench runs an adversarial exploit agent. v2 readiness check #8 says the verifier must "distinguish the current failing state from the required passing state" but has no requirement that a known-good change makes it pass, nor a reward-hack review (e.g. grep-based checks are trivially gamed). Add: (a) verify.sh must fail on base_ref, (b) an oracle branch/patch or at least an author dry-run proves it can pass, (c) verifier writes a machine-readable result file (Harbor's `/logs/verifier/reward.txt` pattern) in addition to `DONE`.
9. **Compaction survival not wired.** v1 mentions post-compaction reinjection; v2 template does not specify the concrete hook. Docs give it exactly: `SessionStart` with `"matcher": "compact"` echoing `task id`, `git status`, `git log --oneline -5`, progress log. Also add a CLAUDE.md line "When compacting, always preserve the task ID, modified file list, and verify command" (best-practices).
10. **Frontmatter `protected_paths` + `task_contract.allowed_mutations` have no consumer.** Hooks and orchestrators need a concrete artifact. Provide the `PreToolUse` script and the outer `diff --ignore-matching-lines` normalizer, or drop the fields.

### Missing

1. **Runtime/launch recipe.** No source-backed invocation for unattended runs. Add: `claude -p "/goal <short condition>" --permission-mode auto|dontAsk --allowedTools "Bash(./verify.sh),Bash(cargo test *),Edit,Read" --max-turns N --max-budget-usd X --output-format stream-json --verbose --bare --append-system-prompt-file .agent/GOAL-TASK-PROTOCOL.md`; for full bypass, run inside the devcontainer (non-root, `init-firewall.sh`) with `--dangerously-skip-permissions`. Codex equivalent: `codex exec --sandbox workspace-write --json -o result.md - < prompt.md`.
2. **Environment/preconditions as executable checks.** Harbor `task.toml` + Dockerfile and SWE-bench `environment_setup_commit` make the environment part of the task. v2 preconditions are prose; add a `preflight` command (or make verify.sh support `./verify.sh --preflight`) whose failure = `BLOCKED` mechanically.
3. **Machine-readable result for the orchestrator.** Claude `--output-format json --json-schema` and Codex `--output-schema` exist; the completion report should be emitted as JSON conforming to a schema (STATUS, acceptance map, verify exit code) so the outer gate parses it instead of regexing prose.
4. **Hints/context field distinct from requirements.** SWE-bench separates `problem_statement` from `hints_text`; Copilot asks for file pointers; Amp "which files to look at, which commands to run". v2 has "Starting point and required context" — good — but should mark it non-normative so the agent does not treat hints as MUSTs.
5. **Turn/idle safeguards and background-task awareness.** `/goal` defers evaluation while background shells run and caps idle check-ins; instruct the executor not to leave dev servers running (headless kills background shells ~5 s after final result).
6. **Task linter.** OpenAI and Harbor enforce doc/task structure mechanically (linters, contributor checklist tools). The v2 author-validation list (26 items) should be a script (frontmatter schema, ID/indent consistency, leaf count, verify.sh executable and failing on base_ref) rather than a comment.
7. **Explicit "hard to verify → escalate" rule.** Sources treat non-deterministic acceptance (UI, "looks good") via screenshots/browser (Anthropic harness, best-practices `/verify`, Cursor artifacts). Template should require a concrete visual/e2e artifact path in the evidence map for any user-visible AC, or mark the task as needing the Layer-3 reviewer.
8. **Evidence that a whole-task DAG + one-task-per-context beats one long session is model-dependent.** Anthropic harness-design notes Opus 4.6 "could natively handle the job without this sort of decomposition". Template docs should state the decomposition size as a tunable, not a constant.
