# Folder discovery demonstration with real experimental tasks

## Provenance

`tasks/demo/pgtui/001` through `007` are complete copies of
`experiments/tasks/TASK-001` through `TASK-007`. Original README, verifier, and
trusted files are preserved byte for byte. New `task.toml` sidecars give each
package its canonical monitor identity and retain the original sequential
predecessor order. Project and group README files describe the demonstration.

The initial catalog contains seven pending demo tasks, zero completed tasks,
and six blocked pending tasks. Only `demo/pgtui/001` is initially ready.

## Supply a project root

The supplied root contains project directories. Projects and groups come from
directory names and README content, not a registration list or creation API.

```text
tasks/                         # supplied root
  demo/                        # project code
    README.md                  # project name and description
    pgtui/                     # group code
      README.md                # group name and description
      001/                     # local task number
        README.md              # original task/v5 contract
        verify.toml            # original verification configuration
        task.toml              # status and canonical dependencies
        trusted/               # original verifier inputs
      002/
      ...
      007/
```

For this repository, start `task-monitor --tasks-root tasks`. To supply another
root, use `task-monitor --tasks-root /absolute/path/to/projects --runs-root
/absolute/path/to/separate-run-state`. Each root must contain the same hierarchy.
Use a separate run root when changing catalogs because run evidence is bound to
its original catalog. See [metadata schema](monitoring.md#metadata-schema).

The server scans the configured folder on each catalog refresh. The browser polls
every three seconds, so new folders appear without restarting the backend.

## Verified against the running application

On 2026-09-07, the seven packages were added while the backend was already running
against `tasks/`. No backend restart, API creation call, or hardcoded project entry
was used. The API discovered a fifth project, `demo`, with one group and seven tasks.

Real Chrome checks against the production frontend confirmed:

- Dashboard: Demo appears with one group and seven tasks.
- `/projects/demo`: the scanned project README and pgtui group appear.
- `/projects/demo/groups/pgtui`: all seven task cards appear, with six blocked.
- Task `007`: complete task content and its dependency on `006` appear.
- `/tasks/my`: all seven demo tasks and all six dependency edges appear.
- No browser page or console errors occurred.

All seven copied packages passed the existing taskfmt lint with zero warnings.
Their source files and trusted assets were compared byte for byte with the
original experimental packages, excluding only the added metadata sidecars.

This test proves folder discovery and display. No coding agent was launched;
the tasks remain pending and no execution success or done status is claimed.
