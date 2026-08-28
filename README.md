# task-format

Research project: which markdown task structure gives the most predictable output from coding agents (Claude Code `/goal`, Codex) when one bounded task is handed to a fresh agent in an isolated Docker container.

Method: prototype one structure at a time, run it in a clean container against a fixture repo, capture the result, analyze, iterate. Every iteration is triggered manually.

## Layout

```text
docs/research/raw/        verbatim research inputs (chronological; later files supersede earlier)
docs/research/notes/      sub-agent research and critique notes
docs/research/RESEARCH-FINDINGS.md   single source of truth (consolidated, deduplicated)
reference/task-template/  current recommended task package (README.md, progress.md, verify.sh, ...)
reference/goal-prompt.md  the prompt used to start the agent
experiments/              fixture repos, run outputs, harness scripts
```

## Status

- v3 reference package written and gate-tested: `reference/task-template/`, example in `reference/example/`, launch prompt in `reference/goal-prompt.md`.
- Decisions + evidence: `docs/research/RESEARCH-FINDINGS.md`.
- Next: build the container harness (`experiments/`), run v3 on Claude Code, then Codex.
