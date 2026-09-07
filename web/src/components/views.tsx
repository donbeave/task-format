import { Link } from "@tanstack/react-router";
import {
  type Catalog,
  excerpt,
  quantity,
  type Task,
  taskHref,
} from "../lib/api";
import { useCatalog } from "../lib/catalog-context";
import {
  Badge,
  Counts,
  Empty,
  labels,
  PageHeading,
  ProgressMeter,
  Readme,
  RunAction,
  Stats,
  TaskCard,
} from "./monitor";

function useSnapshot() {
  const { state } = useCatalog();
  return state.status === "ready" ? state.catalog : null;
}
export function Dashboard() {
  const catalog = useSnapshot();
  if (!catalog) return null;
  const projects = Object.values(catalog.projects);
  return (
    <>
      <PageHeading
        eyebrow="Workspace overview"
        title="Projects"
        description="A clear view of every project, dependency, and verified step forward."
      />
      <Stats summary={catalog.summary} projects={projects.length} />
      <div className="page-heading">
        <h2>All projects</h2>
        <span className="muted">
          {quantity(catalog.summary.task_count, "task")} across{" "}
          {quantity(projects.length, "project")}
        </span>
      </div>
      {projects.length === 0 ? (
        <Empty>
          No projects found. Add project directories to the configured tasks
          root.
        </Empty>
      ) : (
        <div className="grid">
          {projects.map((project) => (
            <div className="frame" key={project.code}>
              <Link
                className="panel panel-link"
                to="/projects/$projectCode"
                params={{ projectCode: project.code }}
              >
                <div className="panel-head">
                  <div className="project-title">
                    <span className="project-icon">
                      {project.name.slice(0, 2)}
                    </span>
                    <h2>{project.name}</h2>
                  </div>
                  <span aria-hidden="true">↗</span>
                </div>
                <p className="muted">
                  {excerpt(project.readme) || "No description provided."}
                </p>
                <p className="muted section">
                  {quantity(project.groups.length, "group")} ·{" "}
                  {quantity(project.summary.task_count, "task")}
                </p>
                <Counts summary={project.summary} />
                <ProgressMeter
                  value={project.summary.progress}
                  label="Overall progress"
                />
              </Link>
            </div>
          ))}
        </div>
      )}
    </>
  );
}
export function ProjectView({ code }: { code: string }) {
  const catalog = useSnapshot();
  if (!catalog) return null;
  const project = catalog.projects[code];
  if (!project) return <Missing />;
  return (
    <>
      <nav className="breadcrumbs" aria-label="Breadcrumb">
        <Link to="/">Projects</Link>
        <span>/</span>
        <span>{project.name}</span>
      </nav>
      <PageHeading eyebrow="Project" title={project.name}>
        <RunAction
          scope={{ kind: "project", id: project.code }}
          eligibility={project.eligibility}
          label="Run project"
        />
      </PageHeading>
      <div className="panel">
        <Readme>{project.readme}</Readme>
      </div>
      <Stats summary={project.summary} />
      <section className="section">
        <h2>
          Groups <span className="muted">{project.groups.length}</span>
        </h2>
        {project.groups.length === 0 ? (
          <Empty>No groups in this project.</Empty>
        ) : (
          <div className="grid">
            {project.groups.map((id) => {
              const group = catalog.groups[id];
              return group ? (
                <div className="frame" key={id}>
                  <Link
                    className="panel panel-link"
                    to="/projects/$projectCode/groups/$groupCode"
                    params={{ projectCode: code, groupCode: group.code }}
                  >
                    <div className="panel-head">
                      <h2>{group.name}</h2>
                      <span className="muted">
                        {quantity(group.summary.task_count, "task")} ↗
                      </span>
                    </div>
                    <p className="muted">
                      {excerpt(group.readme) || "No description provided."}
                    </p>
                    <Counts summary={group.summary} />
                    <ProgressMeter value={group.summary.progress} />
                  </Link>
                </div>
              ) : null;
            })}
          </div>
        )}
      </section>
    </>
  );
}
export function GroupView({
  projectCode,
  groupCode,
}: {
  projectCode: string;
  groupCode: string;
}) {
  const catalog = useSnapshot();
  if (!catalog) return null;
  const group = catalog.groups[`${projectCode}/${groupCode}`];
  if (!group) return <Missing />;
  const tasks = group.tasks.flatMap((id) =>
    catalog.tasks[id] ? [catalog.tasks[id]] : [],
  );
  return (
    <>
      <Breadcrumbs catalog={catalog} projectCode={projectCode} />
      <PageHeading eyebrow="Group · Kanban" title={group.name}>
        <RunAction
          scope={{ kind: "group", id: group.id }}
          eligibility={group.eligibility}
          label="Run group"
        />
      </PageHeading>
      <div className="panel">
        <Readme>{group.readme}</Readme>
      </div>
      <Counts summary={group.summary} />
      <p className="muted mb-3">
        Scroll horizontally to see all statuses. Keyboard: focus the board, then
        use arrow keys.
      </p>
      {/* Keyboard users need focus here to scroll all four columns on narrow screens. */}
      {/* biome-ignore lint/a11y/noNoninteractiveTabindex: focusable horizontal scroll region. */}
      <section className="board" aria-label="Task Kanban" tabIndex={0}>
        {(["draft", "pending", "in_progress", "done"] as const).map(
          (status) => (
            <section
              className="column"
              key={status}
              aria-label={`${labels[status]} tasks`}
            >
              <div className="column-head">
                <Badge status={status} />
                <span className="muted">
                  {
                    tasks.filter((task) => task.metadata.status === status)
                      .length
                  }
                </span>
              </div>
              {tasks
                .filter((task) => task.metadata.status === status)
                .map((task) => (
                  <TaskCard key={task.id} task={task} />
                ))}
              {!tasks.some((task) => task.metadata.status === status) && (
                <Empty>No tasks</Empty>
              )}
            </section>
          ),
        )}
      </section>
    </>
  );
}
function DependencyLinks({
  ids,
  catalog,
}: {
  ids: readonly string[];
  catalog: Catalog;
}) {
  return ids.length ? (
    ids.map((id) => {
      const task = catalog.tasks[id];
      return task ? (
        <a className="dependency" key={id} href={taskHref(task)}>
          {id} · {labels[task.metadata.status]}
          {task.metadata.status === "draft" ? " (ignored)" : ""}
        </a>
      ) : (
        <p key={id}>{id} — unavailable</p>
      );
    })
  ) : (
    <span className="muted">None</span>
  );
}
export function TaskView({
  projectCode,
  groupCode,
  taskNumber,
}: {
  projectCode: string;
  groupCode: string;
  taskNumber: string;
}) {
  const catalog = useSnapshot();
  if (!catalog) return null;
  const task = catalog.tasks[`${projectCode}/${groupCode}/${taskNumber}`];
  if (!task) return <Missing />;
  return (
    <>
      <Breadcrumbs
        catalog={catalog}
        projectCode={projectCode}
        groupCode={groupCode}
      />
      <PageHeading eyebrow={task.id} title={task.title}>
        <RunAction
          scope={{ kind: "task", id: task.id }}
          eligibility={task.eligibility}
          label="Run task"
        />
      </PageHeading>
      <div className="detail-grid">
        <article className="panel">
          <h2>Task contract</h2>
          <Readme>{task.readme}</Readme>
        </article>
        <aside className="panel">
          <dl className="facts">
            <div>
              <dt>Persisted status</dt>
              <dd>
                <Badge status={task.metadata.status} />
              </dd>
            </div>
            <div>
              <dt>Dependency readiness</dt>
              <dd>
                {task.readiness.allowed
                  ? "Ready now"
                  : task.readiness.reason || "Unavailable"}
              </dd>
            </div>
            <div>
              <dt>Checklist</dt>
              <dd>
                <ProgressMeter value={task.progress.percentage} />
                <p className="muted">
                  {task.progress.completed} / {task.progress.total} leaves
                  complete
                </p>
              </dd>
            </div>
            <div>
              <dt>Current checklist leaf</dt>
              <dd>
                {task.progress.current_leaf || "No active checklist leaf"}
              </dd>
            </div>
            <div>
              <dt>Dependencies</dt>
              <dd>
                <DependencyLinks
                  ids={task.metadata.dependencies}
                  catalog={catalog}
                />
              </dd>
            </div>
            <div>
              <dt>Dependents · may run after completion</dt>
              <dd>
                <DependencyLinks ids={task.dependents} catalog={catalog} />
                <p className="muted">
                  All other prerequisites must also finish.
                </p>
              </dd>
            </div>
            <div>
              <dt>Run state</dt>
              <dd>{task.progress.run_state || "No run recorded"}</dd>
            </div>
            <div>
              <dt>Latest event</dt>
              <dd>{task.progress.latest_event || "No event recorded"}</dd>
            </div>
            <div>
              <dt>Current failure</dt>
              <dd>{task.progress.current_failure || "None"}</dd>
            </div>
            <div>
              <dt>Latest run result</dt>
              <dd>
                {task.latest_run
                  ? `${task.latest_run.state}${task.latest_run.failure ? ` — ${task.latest_run.failure}` : ""}`
                  : task.metadata.status === "done"
                    ? "Authoritatively verified"
                    : task.progress.run_state || "No result recorded"}
              </dd>
            </div>
          </dl>
        </aside>
      </div>
    </>
  );
}
export function dependencyStages(
  catalog: Catalog,
): readonly (readonly Task[])[] {
  const depth = new Map<string, number>();
  const stages: Task[][] = [];
  for (const id of catalog.topological_order) {
    const task = catalog.tasks[id];
    if (!task || task.metadata.status === "draft") continue;
    const level = Math.max(
      0,
      ...task.metadata.dependencies
        .filter((dep) => catalog.tasks[dep]?.metadata.status !== "draft")
        .map((dep) => (depth.get(dep) ?? -1) + 1),
    );
    depth.set(id, level);
    stages[level] ??= [];
    stages[level].push(task);
  }
  return stages;
}
export function MyTasks() {
  const catalog = useSnapshot();
  if (!catalog) return null;
  const stages = dependencyStages(catalog);
  const tasks = Object.values(catalog.tasks).filter(
    (task) => task.metadata.status !== "draft",
  );
  const ready = tasks.filter((task) => task.readiness.allowed);
  return (
    <>
      <PageHeading
        eyebrow="Tasks / My tasks"
        title="What’s next"
        description="Follow dependencies across projects. Tasks in the same stage form independent branches; readiness comes from the backend."
      />
      <div className="panel">
        <div className="panel-head">
          <h2>Ready now</h2>
          <span className="badge">{quantity(ready.length, "task")}</span>
        </div>
        {ready.length ? (
          <div className="lane-cards">
            {ready.map((task) => (
              <TaskCard key={task.id} task={task} />
            ))}
          </div>
        ) : (
          <Empty>
            No tasks ready now. Check blocked prerequisites or execution
            availability below.
          </Empty>
        )}
      </div>
      <section className="section">
        <h2>Dependency path</h2>
        <p className="muted">
          Each task lists its exact prerequisites and the tasks it may unlock.
          Draft tasks are excluded.
        </p>
        {stages.length === 0 ? (
          <Empty>No non-draft tasks found.</Empty>
        ) : (
          stages.map((tasksInStage, index) => (
            <div
              className="lane"
              key={tasksInStage.map((task) => task.id).join(",")}
            >
              <div className="lane-label">
                Stage {index + 1}
                <p>
                  {tasksInStage.length} parallel{" "}
                  {tasksInStage.length === 1 ? "branch" : "branches"}
                </p>
              </div>
              <div className="lane-cards">
                {tasksInStage.map((task) => (
                  <TaskCard key={task.id} task={task}>
                    <div className="section">
                      <p className="muted">Requires</p>
                      <DependencyLinks
                        ids={task.metadata.dependencies}
                        catalog={catalog}
                      />
                      <p className="muted">May unlock</p>
                      <DependencyLinks
                        ids={task.dependents}
                        catalog={catalog}
                      />
                    </div>
                  </TaskCard>
                ))}
              </div>
            </div>
          ))
        )}
      </section>
    </>
  );
}
function Breadcrumbs({
  catalog,
  projectCode,
  groupCode,
}: {
  catalog: Catalog;
  projectCode: string;
  groupCode?: string;
}) {
  return (
    <nav className="breadcrumbs" aria-label="Breadcrumb">
      <Link to="/">Projects</Link>
      <span>/</span>
      <Link to="/projects/$projectCode" params={{ projectCode }}>
        {catalog.projects[projectCode]?.name || projectCode}
      </Link>
      {groupCode && (
        <>
          <span>/</span>
          <Link
            to="/projects/$projectCode/groups/$groupCode"
            params={{ projectCode, groupCode }}
          >
            {catalog.groups[`${projectCode}/${groupCode}`]?.name || groupCode}
          </Link>
        </>
      )}
    </nav>
  );
}
function Missing() {
  return (
    <>
      <PageHeading
        eyebrow="Not found"
        title="This record is unavailable"
        description="The directory may have moved or no longer exists."
      />
      <Link to="/">Return to projects</Link>
    </>
  );
}
