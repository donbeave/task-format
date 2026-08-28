# experiments

Not built yet. Headed design (herdr) in `docs/research/notes/headed-herdr-harness.md`; image/mount/gate basics in `docs/research/notes/container-harness.md`; decisions in `docs/research/RESEARCH-FINDINGS.md` §5.

Planned layout:

```text
images/{claude,codex}/Dockerfile   images/common/{init-firewall.sh,entrypoint.sh}
fixtures/<name>/                    small repos the tasks act on
tasks/<TASK-ID>/                    task.md AGENT.md verify.sh verify.config protected.sha256 fixture prompt.txt
run-headed.sh                       run-headed.sh <claude|codex> <TASK-ID> [--model M] [--net api|all]  (persistent container, herdr)
attach.sh / status.sh               re-attach to the live agent; detect completion
lib/                                hash-protected.sh, gate.sh
runs/<ts>-<agent>-<task>/           workspace, task-snapshot, progress.md, agent-home, transcript.jsonl, stdout.ndjson, diff.patch, verify-host.log, metrics.json
```
