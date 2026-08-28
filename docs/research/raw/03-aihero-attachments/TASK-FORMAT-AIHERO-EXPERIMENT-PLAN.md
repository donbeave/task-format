# Experimental validation plan for the AIHero-inspired changes

The proposed changes are hypotheses. `task-format` should retain its
one-variable-at-a-time research method.

## Execution-template ablations

Use identical fixture, base commit, model, seed distribution, container,
network policy, budget, turn cap, and host verifier.

| Variant | Change |
| --- | --- |
| E0 | Current `task/v3` |
| E1 | E0 + demo path and independent value |
| E2 | E0 + primary/supporting verification seams |
| E3 | E0 + AC class and baseline/final expectations |
| E4 | E0 + decision provenance/binding-source split |
| E5 | All proposed direct additions |

Minimum sample:

- all six pgtui task kinds;
- at least five independent runs per task/model condition;
- Claude Code and Codex in separate result strata;
- unchanged trusted verifier and reference solution.

Measure:

- host-gate pass rate;
- first-pass verifier success;
- number of verifier reruns;
- turns, tokens, wall time, and cost;
- out-of-scope file attempts;
- protected-file tamper attempts;
- lower-seam/implementation-coupled test substitutions;
- ACs checked without matching evidence;
- demo-path execution rate;
- independent-review finding count and severity;
- terminal `BLOCKED`/`NEEDS_REPLAN` correctness.

## Decomposition experiment

Input the same settled multi-feature specification to multiple fresh planner
contexts.

Variants:

1. Current authoring instructions.
2. Tracer-bullet rule only.
3. Tracer-bullet + required demo path.
4. Tracer-bullet + demo path + blocking edges.
5. Full proposed author gate and graph schema.

Blind reviewers score each generated graph:

- percentage of tasks with an independent demo;
- percentage of horizontal/layer-only tasks;
- ACs that depend on future tasks;
- blocker-edge precision and recall;
- cycles or hidden dependencies;
- duplicate requirement ownership;
- tasks likely to exceed one context;
- unnecessary task count/over-decomposition;
- prefactor usefulness;
- expand-contract sequence closure;
- total compiled task token size.

## Adoption rule

Adopt an individual change only when it improves gate pass rate or review
quality without a material increase in context size, turns, or false
`NEEDS_REPLAN` outcomes.

Do not make `task/v3.1` the default based only on subjective template quality.
