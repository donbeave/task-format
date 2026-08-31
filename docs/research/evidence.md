# Evidence

## Verdict

The host-side `taskfmt gate` is the success authority. Agent narration,
checklist state, and a clean-looking diff are diagnostic evidence only.

## Record for every run

- task variant, agent profile, image, and runtime configuration;
- starting baseline and resulting diff;
- gate commands, output, and verdict;
- completion claims that disagree with the gate;
- scope violations, retries, and rework; and
- logs and snapshots needed to explain the result.

Run artifacts belong under `experiments/runs/` and are generated, not hand-
edited documentation.

## Compare

Report at least gate pass rate, false-completion rate, scope violations, and
diff stability across repeated runs. Report failures and spread, not just the
best run.

## Adoption bar

Adopt a format change only after repeated control comparisons show a useful,
reproducible improvement without weakening verification or changing the task
outcome. Record the decision and its limits in this folder.

## Limits

The current repository demonstrates that the lifecycle can run. It does not
yet establish causal effects across models, providers, or task domains.
