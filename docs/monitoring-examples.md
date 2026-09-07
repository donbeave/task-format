# Monitoring example catalog

`tasks/` contains four representative projects: Jackin, ChainArgos, Parallax, and Velnor. Their names are display metadata, not claims about those products. All executable task contracts concern the existing pgtui experiment. Nothing in `experiments/tasks/` was changed.

## Provenance and contents

| Hierarchical package | Original package | Initial status | Dependency |
| --- | --- | --- | --- |
| `jackin/new-design/001` | `experiments/tasks/TASK-001` | pending | none |
| `jackin/new-design/002` | `experiments/tasks/TASK-002` | pending | `jackin/new-design/001` |
| `jackin/new-design/003` | `experiments/tasks/TASK-003` | pending | `jackin/new-design/002` |
| `jackin/new-design/004` | `experiments/tasks/TASK-004` | pending | `jackin/new-design/003` |
| `jackin/new-design/005` | `experiments/tasks/TASK-005` | pending | `jackin/new-design/004` |
| `jackin/new-design/006` | `experiments/tasks/TASK-006` | pending | `jackin/new-design/005` |
| `jackin/new-design/007` | `experiments/tasks/TASK-007` | pending | `jackin/new-design/006` |
| `chainargos/bootstrap/001` | `experiments/tasks/TASK-001` | pending | none |
| `parallax/bootstrap/001` | `experiments/tasks/TASK-001` | pending | none |
| `velnor/bootstrap/001` | `experiments/tasks/TASK-001` | draft | none |

Each task preserves its original `README.md`, `verify.toml`, and complete `trusted/` directory byte for byte. The render implementation, font, and behavioral tests are required verifier inputs, not unused UI assets. Only the versioned `task.toml` sidecar is added inside task packages. Internal contract IDs such as `TASK-001` stay unchanged; canonical catalog IDs use the project/group/number path, so independently owned copies cannot collide.

Jackin intentionally retains the original sequential preconditions. Independent pending roots in ChainArgos and Parallax demonstrate parallel readiness without pretending that later pgtui slices can run before their baselines exist. Scheduler tests cover branching and joining within a project separately.

The initial catalog has four projects, four groups, ten tasks, one draft, nine pending (six blocked), zero in progress, zero done, and zero percent checklist progress. Its ready tasks are `jackin/new-design/001`, `chainargos/bootstrap/001`, and `parallax/bootstrap/001`. Blocked is a computed subset of pending, not an extra persisted status.

## Running the fixtures

These are real task contracts, not instant-success demos. A configured execution adapter must invoke the existing `taskfmt` lifecycle with the appropriate pgtui execution repository and baseline. TASK-001 requires an empty implementation baseline plus its trusted scaffold; later tasks require the preceding completed implementation. A dependency reaching done establishes scheduling eligibility but does not manufacture a workspace baseline. Independent bootstrap copies need independent empty baselines, not the already bootstrapped Jackin repository.

The original checks require the pinned toolchain, Cargo dependencies, trusted assets, and, for the database slices, the existing PostgreSQL/container prerequisites. Follow the harness setup and execution documentation before enabling execution. Missing execution configuration must remain visible as an unavailable action in the monitoring UI.

Do not mark these packages done to decorate the dashboard. Only successful authoritative verification may do that. Runtime status and progress belong to local execution state; reset or clone fixtures explicitly when a fresh demonstration catalog is needed.

## Compatibility checks

Lint the packages with the existing CLI, passing package directories:

```sh
rtk cargo run --manifest-path harness/Cargo.toml --bin taskfmt -- lint tasks/jackin/new-design/001 tasks/jackin/new-design/002 tasks/jackin/new-design/003 tasks/jackin/new-design/004 tasks/jackin/new-design/005 tasks/jackin/new-design/006 tasks/jackin/new-design/007 tasks/chainargos/bootstrap/001 tasks/parallax/bootstrap/001 tasks/velnor/bootstrap/001
```

Lint validates contracts and verification manifests. It does not execute the pgtui implementation checks and does not establish done status.

## Preserved font license

The unchanged `trusted/crates/pgtui/src/fonts/DejaVuSansMono.ttf` copies contain the following license in their embedded metadata. It is reproduced here for inspection. The font is a protected pgtui renderer input; the monitoring frontend does not load it.

Fonts are (c) Bitstream (see below). DejaVu changes are in public domain.

Bitstream Vera Fonts Copyright

Copyright (c) 2003 by Bitstream, Inc. All Rights Reserved. Bitstream Vera is a trademark of Bitstream, Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy of the fonts accompanying this license ("Fonts") and associated documentation files (the "Font Software"), to reproduce and distribute the Font Software, including without limitation the rights to use, copy, merge, publish, distribute, and/or sell copies of the Font Software, and to permit persons to whom the Font Software is furnished to do so, subject to the following conditions:

The above copyright and trademark notices and this permission notice shall be included in all copies of one or more of the Font Software typefaces.

The Font Software may be modified, altered, or added to, and in particular the designs of glyphs or characters in the Fonts may be modified and additional glyphs or  or characters may be added to the Fonts, only if the fonts are renamed to names not containing either the words "Bitstream" or the word "Vera".

This License becomes null and void to the extent applicable to Fonts or Font Software that has been modified and is distributed under the "Bitstream Vera" names.

The Font Software may be sold as part of a larger software package but no copy of one or more of the Font Software typefaces may be sold by itself.

THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL BITSTREAM OR THE GNOME FOUNDATION BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM OTHER DEALINGS IN THE FONT SOFTWARE.

Except as contained in this notice, the names of Gnome, the Gnome Foundation, and Bitstream Inc., shall not be used in advertising or otherwise to promote the sale, use or other dealings in this Font Software without prior written authorization from the Gnome Foundation or Bitstream Inc., respectively. For further information, contact: fonts at gnome dot org.
