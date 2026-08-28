# task-format

Research project: which markdown task structure gives coding agents the most predictable output. Source of truth: `docs/research/RESEARCH-FINDINGS.md`. Reference package: `reference/task-template/`.

## Hard rules

- Terminal multiplexer for headed agent runs is **herdr** (https://herdr.dev/) only. Never use or propose tmux, zellij, screen, or any other multiplexer.
- Agents run headed (interactive TUI) inside persistent Docker containers (no `--rm`); the operator re-attaches later to inspect the live session and container state.
- Every experiment run is triggered manually by the operator; no autonomous orchestration.
- `README.md`, `AGENTS.md`, `verify.sh` are read-only for the executing agent; only `progress.md` is agent-writable.
- `progress.md` is never committed. It is generated per run from `README.md` by `reference/task-template/progress-init.sh` (`.gitignore` enforces this).
- After editing any script in `reference/task-template/`, run `reference/task-template/selftest.sh`; task packages must pass `task-lint.sh`.
- Agent instruction files are named `AGENTS.md`; every `AGENTS.md` has a sibling `CLAUDE.md` that is a relative symlink to it (`ln -s AGENTS.md CLAUDE.md`). Never a real `CLAUDE.md`, never the reverse direction. `selftest.sh` checks this repo-wide.
- Commits use DCO signoff (`git commit -s`).
