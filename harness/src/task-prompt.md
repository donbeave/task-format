```text claude codex
/goal Implement the single task described in `@/task/README.md`, following `@/task/AGENTS.md`
exactly. Before editing, read both files fully. Work only in `/work`; nothing under `/task/` may
change. Done when, after the last file change, `taskfmt verify` has been run from `/work`, exited
`0`, and printed `DONE` as its last line, with the complete command and output shown in the
transcript; `/progress/progress.md` has a valid terminal `DONE` event stream; and the final report
ending in a `GOAL_RESULT` line has been printed. Do not change any file outside the `writable_paths`
list in `/task/verify.toml`. Do not weaken, skip, delete, or bypass checks. If a precondition
fails, stop with `STATUS: BLOCKED`; if the task needs a scope, decision, or checklist change, stop
with `STATUS: NEEDS_REPLAN`. Stop after 40 turns; if stopping at the cap, leave `STATE: IN_PROGRESS`
and print `STATUS: INCOMPLETE`.
```
