# Why structured tasks

Coding agents act from written context. A task can describe the same desired software outcome in many ways: a short request, a checklist, a detailed contract, or a mixture of instructions and verification. Those forms can change what an agent notices, edits, verifies, and reports.

Advice about writing tasks is common, but controlled evidence is scarce. “Be specific” is not enough when the goal is reliable autonomous work. This project tests task-package structure as an engineering variable.

## What makes this hard in the AI world

An agent starts with incomplete knowledge of a repository and must turn natural language into a sequence of edits, checks, and decisions. Small ambiguities can produce large differences:

- An unclear boundary can expand the diff beyond intended behavior.
- Missing verification can let an agent report completion without proving it.
- Unstable starting state can make two runs incomparable.
- Agent narration can sound convincing while the implementation is wrong.

The project therefore treats an agent run as an experiment, not a demonstration.

## What the project tests

For a bounded software change, task-format keeps the software goal and execution environment comparable while varying how the task package is written. Each run starts with fresh state, ends with an independent machine check, and records results for later comparison.

Predictability is measured from harness evidence: pass rate, false completion claims, scope violations, and stability of resulting diffs across repeated runs. It is not inferred from an agent’s confidence or a human impression.

## What this project is not

It does not rank models, claim one universal task-writing style, or replace engineering judgment. It builds a repeatable way to test specific task-writing choices and retain only changes supported by results.
