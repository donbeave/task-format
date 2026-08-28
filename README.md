# task-format

Research project: which markdown task structure gives the most predictable output from coding agents (Claude Code `/goal`, Codex) when one bounded task is handed to a fresh agent in a fresh Docker container working on a fresh clone of the experiment repo.

Method: prototype one structure at a time, run it end to end — from a truly empty GitHub repo `main` to the complete example app, one bounded task at a time, gate-checked and pushed only on PASS — capture the result, analyze, iterate.

## Layout

```text
docs/research/raw/        verbatim research inputs (chronological; later files supersede earlier)
docs/research/notes/      sub-agent research and critique notes
docs/research/RESEARCH-FINDINGS.md   single source of truth (consolidated, deduplicated)
reference/task-template/  the task package — pure template (README.md, AGENTS.md, verify.toml); nothing else
harness/                  the Rust CLI (binary `taskfmt`): lint, progress-init, gate, selftest,
                          image build/preload, repo lifecycle, dispatch, attach/status, experiment loop,
                          container entrypoint/prereqs/agent-launch; plus goal-prompt.md and the lint corpus
experiments/              task packages (TASK-001..007 with trusted/ verification material), seed data,
                          run outputs — data only
experiment.toml           versioned experiment manifest (github, images, runtime, agent profiles)
```

## Status

- v4 reference package (schema task/v4): task content in `README.md`, protocol in `AGENTS.md` (+`CLAUDE.md` symlink), declarative gate config `verify.toml`; the gate is the baked-in `taskfmt verify` binary.
- Task sequence TASK-001..007 starts at repository bootstrap from an empty `main` and ends with the complete `pgtui` app (Rust ratatui PostgreSQL browser). No task assumes fixture-supplied application code; each package ships `trusted/` verification material overlaid at dispatch (D25, D28).
- Execution tooling is one Rust CLI, `taskfmt` (D24). No shell scripts in the execution path. Container images bake the binary; the entrypoint/prereq stage is Rust.
- Lifecycle (D25): disposable GitHub repo with allow-empty bootstrap `main`; per task fresh clone → trusted overlay commit (never pushed) → headed herdr run in a persistent privileged container → host gate → `git commit -s` + push only on PASS.
- Agents run through the `zai-flash` profile: Z.ai Anthropic-compatible endpoint, GLM-5.3-Flash, effort low, token from 1Password via `op read`, injected by env-file, redacted everywhere (D27).
- Decisions + evidence: `docs/research/RESEARCH-FINDINGS.md` (D1–D30).
