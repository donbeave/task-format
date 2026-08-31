# Shared fixtures

This directory contains static inputs shared by experiment runs. It is intentionally small: task-specific verification material lives with its task in `../tasks/TASK-*/trusted/`, where it can grow with the task series without becoming agent-authored application code.

## Default PostgreSQL seed

[`seed/pgtui-seed.sql`](seed/pgtui-seed.sql) is the default database seed. `experiment.toml` names `experiments/fixtures/seed` as `seed_dir`; dispatch copies that directory into the run and mounts it read-only at `/seed` inside the container.

The prerequisite stage restores any repository `tests/fixtures/seed.sql` files first, then the sorted SQL files from `/seed`. The default seed is therefore shared by every run and must stay compatible with the task-owned test seed.

The current seed creates:

| Schema | Table | Rows | Purpose |
| --- | --- | ---: | --- |
| `public` | `customers` | 5 | Text, decimal, date, duplicate-value, and `NULL` coverage. |
| `public` | `orders` | 7 | Related rows and varied statuses. |
| `public` | `empty_table` | 0 | Empty-result coverage. |
| `audit` | `events` | 3 | A second schema and timestamp values. |

The SQL starts by dropping the objects it owns, then recreates them. It is designed for a fresh prerequisite database, not as a general migration file.

## Change rules

- Keep this seed deterministic and compatible with `tests/fixtures/seed.sql` shipped in later task packages.
- Preserve the table names, columns, row values, `NULL`s, and ordering assumptions unless the affected task contracts and trusted tests change together.
- Do not add application source or task-specific tests here. Put those files in the relevant task package's `trusted/` tree.

For the harness fixture-copying, mounting, and restore behavior, see [`harness/README.md`](../../harness/README.md).
