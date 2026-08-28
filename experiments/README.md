# experiments

Not built yet. Design in `docs/research/notes/container-harness.md`, decisions in `docs/research/RESEARCH-FINDINGS.md` §5.

Planned layout:

```text
images/{claude,codex}/Dockerfile   images/common/{init-firewall.sh,entrypoint.sh}
fixtures/<name>/                    small repos the tasks act on
tasks/<TASK-ID>/                    task.md AGENT.md verify.sh verify.config protected.sha256 fixture prompt.txt
run.sh                              run.sh <claude|codex> <TASK-ID> [--model M] [--net api|all] [--max-turns N] [--budget USD]
lib/                                hash-protected.sh, gate.sh
runs/<ts>-<agent>-<task>/           workspace, task-snapshot, progress.md, agent-home, transcript.jsonl, stdout.ndjson, diff.patch, verify-host.log, metrics.json
```
