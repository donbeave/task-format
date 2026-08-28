# task-format

Research project: which markdown task structure gives coding agents the most predictable output. Source of truth: `docs/research/RESEARCH-FINDINGS.md`. Reference package: `reference/task-template/`.

## Hard rules

- Terminal multiplexer for headed agent runs is **herdr** (https://herdr.dev/) only. Never use or propose tmux, zellij, screen, or any other multiplexer.
- Agents run headed (interactive TUI) inside persistent Docker containers (no `--rm`); the operator re-attaches later to inspect the live session and container state.
- Every experiment run is triggered manually by the operator; no autonomous orchestration.
- `task.md`, `AGENT.md`, `verify.sh` are read-only for the executing agent; only `progress.md` is agent-writable.
- Commits use DCO signoff (`git commit -s`).
