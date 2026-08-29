# task-format

Research project: which markdown task structure gives coding agents the most predictable output. Source of truth: `docs/research/RESEARCH-FINDINGS.md`. Reference package: `reference/task-template/` (pure task files, schema task/v4). Execution tooling: the Rust CLI in `harness/` (binary `taskfmt`).

## Hard rules

- Terminal multiplexer for headed agent runs is **herdr** (https://herdr.dev/) only. Never use or propose tmux, zellij, screen, or any other multiplexer.
- Agents run headed (interactive TUI) inside persistent Docker containers (no `--rm`); the operator re-attaches later to inspect the live session and container state.
- **One task = one fresh container = one fresh clone.** Every task run starts from a newly fetched `origin/main` of the experiment repo (a disposable GitHub repo whose `main` begins as one allow-empty bootstrap commit); leftover workspace state is never reused.
- All execution tooling is the Rust CLI `taskfmt` (`harness/` crate). **No executable shell script in the execution path** — lint, progress generation, gate, selftest, image build/preload, dispatch, attach/status, experiment loop, and the container entrypoint/prereqs/agent-launch are Rust; Dockerfiles and declarative config/data (`verify.toml`, `experiment.toml`, `goal-prompt.md`) are fine.
- Orchestration is deterministic and gate-protected: `taskfmt experiment --auto` may run the whole sequence unattended (supersedes the earlier manual-trigger-only rule, D26); interactive confirmation is the default for every mutating step, and only a PASSing gate may `git commit -s` + push `main`. A failed task never pushes.
- `/task` is read-only for the executing agent (`README.md`, `AGENTS.md`, `verify.toml`, optionally `decisions.md`); the gate is the baked-in `taskfmt verify` binary; only `progress.md` is agent-writable. `taskfmt selfcheck` is an author-side tool (checks the package before dispatch), not part of the agent protocol.
- `progress.md` is never committed. It is generated per run from `README.md` by `taskfmt progress-init` (`.gitignore` enforces this).
- After editing anything in `harness/`, run `taskfmt selftest` (plus `cargo fmt --check && cargo clippy -D warnings && cargo test`); task packages must pass `taskfmt lint`.
- Agent instruction files are named `AGENTS.md`; every `AGENTS.md` has a sibling `CLAUDE.md` that is a relative symlink to it (`ln -s AGENTS.md CLAUDE.md`). Never a real `CLAUDE.md`, never the reverse direction. `taskfmt selftest` checks this repo-wide.
- Provider credentials are `env_secret` **references** in `experiment.toml`, resolved at dispatch by `harness/src/ops/op.rs`. Two schemes: `file://NAME` — a bare file name resolving to a regular file, mode exactly 0600, lying **directly** inside `$HOME/.config/taskfmt/` (parent-equality containment, no subprocess) — and `op://…`, read through `op read`. The `zai-flash` profile (Z.ai endpoint, GLM-5.3-Flash, effort low) uses `file://zai-flash.token`. A resolved value is registered with the redactor and injected only through a 0600 `--env-file` deleted right after `docker run`; it never appears in argv, logs, transcripts, or committed artifacts, and a credential **value** never appears in a tracked file.
- Commits use DCO signoff (`git commit -s`).
- **All work happens directly on `main`.** No feature branches, no pull requests, no worktrees: commit to `main` with `git commit -s` and push. This includes the self-host meta-tasks in `selfhost/` — they are worked in the `main` checkout, one at a time, and committed to `main` like everything else.
