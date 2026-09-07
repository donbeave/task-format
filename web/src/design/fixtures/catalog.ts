import type { Catalog, Status, Summary, Task } from "../../lib/api";

const disabled = {
  allowed: false,
  reason: "Design fixture — execution disabled.",
};
const emptySummary: Summary = {
  task_count: 0,
  draft: 0,
  pending: 0,
  in_progress: 0,
  done: 0,
  blocked: 0,
  progress: 0,
};
const summary: Summary = {
  task_count: 5,
  draft: 1,
  pending: 2,
  in_progress: 1,
  done: 1,
  blocked: 1,
  progress: 30,
};
const makeTask = (
  number: string,
  title: string,
  status: Status,
  dependencies: string[] = [],
): Task => ({
  id: `atlas/interface/${number}`,
  project_code: "atlas",
  group_code: "interface",
  number,
  title,
  contract_id: `DESIGN-${number}`,
  readme: `# ${title}\n\nSynthetic interface fixture. No execution or repository changes.\n\n## Acceptance\n\n- [x] Structure defined\n- [ ] Keyboard navigation verified\n\nUnicode content: Fjörður · 日本語 · Nguyễn.`,
  metadata: { schema: "task-meta/v1", status, dependencies },
  dependents: [],
  progress: {
    completed: status === "done" ? 4 : status === "in_progress" ? 2 : 0,
    total: 4,
    percentage: status === "done" ? 100 : status === "in_progress" ? 50 : 0,
    current_leaf: status === "in_progress" ? "Keyboard navigation" : null,
    current_failure: null,
    run_state: status === "in_progress" ? "running" : null,
    latest_event: null,
  },
  eligibility: disabled,
  readiness:
    status === "pending" && dependencies.length === 0
      ? { allowed: true, reason: null }
      : {
          allowed: false,
          reason: dependencies.length
            ? "Waiting for atlas/interface/002"
            : "Task is not pending.",
        },
  latest_run: null,
});
const tasks = [
  makeTask("001", "Establish the shared workspace foundation", "done"),
  makeTask(
    "002",
    "Refine keyboard navigation — Fjörður and 日本語",
    "in_progress",
  ),
  makeTask(
    "003",
    "A deliberately long task title for responsive wrapping across narrow mobile viewports without losing the task identity",
    "pending",
    ["atlas/interface/002"],
  ),
  makeTask("004", "Review accessible empty states", "draft"),
  makeTask("005", "Verify focus order — Nguyễn", "pending"),
];

export const designCatalog: Catalog = {
  projects: {
    atlas: {
      code: "atlas",
      name: "Atlas — Interface laboratory",
      readme:
        "# Atlas\n\nA synthetic project for reviewing the task monitor interface.",
      groups: ["atlas/interface"],
      summary,
      eligibility: disabled,
    },
  },
  groups: {
    "atlas/interface": {
      id: "atlas/interface",
      project_code: "atlas",
      code: "interface",
      name: "Interface foundations",
      readme:
        "# Interface foundations\n\nShared patterns, accessible navigation, and verification contracts.",
      tasks: tasks.map((task) => task.id),
      summary,
      eligibility: disabled,
    },
  },
  tasks: Object.fromEntries(tasks.map((task) => [task.id, task])),
  summary,
  execution: { available: false, reason: disabled.reason },
  topological_order: tasks.map((task) => task.id),
};

// Keep existing project/group identities so empty children render their real empty state.
export const emptyDesignCatalog: Catalog = {
  ...designCatalog,
  projects: Object.fromEntries(
    Object.entries(designCatalog.projects).map(([key, project]) => [
      key,
      { ...project, groups: [], summary: emptySummary },
    ]),
  ),
  groups: Object.fromEntries(
    Object.entries(designCatalog.groups).map(([key, group]) => [
      key,
      { ...group, tasks: [], summary: emptySummary },
    ]),
  ),
  tasks: {},
  summary: emptySummary,
  topological_order: [],
};

export const emptyWorkspaceCatalog: Catalog = {
  ...emptyDesignCatalog,
  projects: {},
  groups: {},
};
export const designError =
  "The catalog folder could not be read. Check that projects/ exists and is readable, then retry.";
