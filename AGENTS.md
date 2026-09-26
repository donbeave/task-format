# Rules

- No legacy code. Complete migrations by removing obsolete paths; add no compatibility shims, aliases, or deprecation periods.
- Judge changes by correctness, consistency, and project fit. Do not defer known-wrong states due to cost, effort, marginal value, or edge-case status. Inspect, test, or measure when uncertain.
- Before fixing bugs, identify the architecture that enabled them and whether it permits related bugs. Prefer removing the cause; use symptom-level fixes only when a root fix is infeasible or separate, and name the deferred cause.
- Do not reintroduce removed platform integrations.
- Delegate independent work for research, implementation, review, and verification; integrate and check results.
- Work on `main` only. Update from `origin/main`; commit and push verified changes with `git commit -s` and `Co-authored-by: Codex <codex@openai.com>`.
- Keep these instructions lean; put plans and progress in project documentation.
