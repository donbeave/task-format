import type { Catalog, Status, Task } from "../lib/api";

const allowed = { allowed: true, reason: null };
const denied = { allowed: false, reason: "Waiting for jackin/design/001" };
export function fixture(): Catalog {
  const make = (
    number: string,
    status: Status,
    dependencies: string[] = [],
  ): Task => ({
    id: `jackin/design/${number}`,
    project_code: "jackin",
    group_code: "design",
    number,
    title: `Task ${number}`,
    contract_id: `TASK-${number}`,
    readme: `# Task ${number}\n\nComplete task contract.\n\n- [x] First check\n- [ ] Next check`,
    metadata: { schema: "task-meta/v1", status, dependencies },
    dependents: [],
    progress: {
      completed: 1,
      total: 2,
      percentage: status === "done" ? 100 : 50,
      current_leaf: "Next check",
      current_failure: null,
      latest_event: null,
      run_state: null,
    },
    eligibility: dependencies.length ? denied : allowed,
    readiness: dependencies.length ? denied : allowed,
    latest_run: null,
  });
  const a = make("001", "pending"),
    b = make("002", "pending", [a.id]),
    c = make("003", "pending", [a.id]),
    d = make("004", "pending", [b.id, c.id]),
    draft = make("005", "draft");
  a.dependents = [b.id, c.id];
  b.dependents = [d.id];
  c.dependents = [d.id];
  draft.eligibility = { allowed: false, reason: "Draft tasks never execute" };
  draft.readiness = draft.eligibility;
  const tasks = Object.fromEntries(
    [a, b, c, d, draft].map((task) => [task.id, task]),
  );
  const summary = {
    task_count: 5,
    draft: 1,
    pending: 4,
    in_progress: 0,
    done: 0,
    blocked: 3,
    progress: 50,
  };
  return {
    projects: {
      jackin: {
        code: "jackin",
        name: "Jackin",
        readme: "# Jackin\n\nProject description.",
        groups: ["jackin/design"],
        summary,
        eligibility: allowed,
      },
    },
    groups: {
      "jackin/design": {
        id: "jackin/design",
        project_code: "jackin",
        code: "design",
        name: "Design",
        readme: "# Design\n\nGroup description.",
        tasks: Object.keys(tasks),
        summary,
        eligibility: allowed,
      },
    },
    tasks,
    summary,
    execution: { available: true, reason: null },
    topological_order: [a.id, b.id, c.id, d.id, draft.id],
  };
}
