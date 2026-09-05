# Phase 6 proof artifacts

`TASK-001` through `TASK-007` need external implementation evidence. The
repository intentionally carries only task contracts and trusted verification
inputs; `trusted/` is not a reference solution.

Supply one Git repository (bare or non-bare) with these named tags:

```
refs/tags/taskfmt/TASK-001/baseline
refs/tags/taskfmt/TASK-001/reference
...
refs/tags/taskfmt/TASK-007/baseline
refs/tags/taskfmt/TASK-007/reference
```

For every task, both tags must resolve to commits; `reference` must descend
from its paired `baseline`. The baseline commit must contain every file from
that task package's `trusted/` directory at the same relative path and exact
blob identity. The reference commit must preserve those blobs. This makes the
planner-owned oracle part of the immutable baseline, never a mutable solution
input.

Run:

```sh
bash experiments/corpus-preflight.sh /path/to/pgtui-proof.git
```

The command prints the resolved commit OIDs, derives a binary patch from each
`baseline..reference`, runs the
source-built `taskfmt selfcheck`, and fails closed on missing tags, wrong
trusted blobs, broken ancestry, or an unproven RED/GREEN result. A PASS is the
required baseline-red/reference-green evidence; missing artifacts are not a
passing substitute.
