# task-format

Research project: which markdown task structure gives the most predictable output from coding agents (Claude Code `/goal`, Codex) when one bounded task is handed to a fresh agent in an isolated Docker container.

Method: prototype one structure at a time, run it in a clean container against a fixture repo, capture the result, analyze, iterate. Every iteration is triggered manually.

## Layout

```text
docs/research/raw/        verbatim research inputs (chronological; later files supersede earlier)
docs/research/notes/      sub-agent research and critique notes
docs/research/RESEARCH-FINDINGS.md   single source of truth (consolidated, deduplicated)
reference/task-template/  the task package — pure template (README.md, AGENTS.md, verify.sh, verify.config); nothing else
harness/                  scripts and files required to author, dispatch and gate a task (task-lint.sh, progress-init.sh, selftest.sh, goal-prompt.md, testdata/example/, images/ + build.sh/preload.sh, run-headed.sh/attach.sh/status.sh)
experiments/              fixture repos, task packages, run outputs — data only
```

## Status

- v3 reference package written and gate-tested: `reference/task-template/` (pure task files), dispatch tooling in `harness/` (incl. `goal-prompt.md` and the `testdata/example/` lint corpus). `harness/selftest.sh` proves lint, progress generation and the gate.
- `progress.md` is never stored: `harness/progress-init.sh` derives it from `README.md` per run.
- Decisions + evidence: `docs/research/RESEARCH-FINDINGS.md`.
- Prereq container harness built (`harness/images/` + `build.sh`/`preload.sh`, per-run `run-headed.sh`/`attach.sh`/`status.sh`): DinD per run + standing seeded `prereq-postgres` (D19-D20). Next: first headed agent runs on Claude Code, then Codex.
