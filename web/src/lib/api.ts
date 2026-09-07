import { z } from "zod";
export const statusSchema = z.enum(["draft", "pending", "in_progress", "done"]);
const eligibility = z.object({
  allowed: z.boolean(),
  reason: z.string().nullable(),
});
const summary = z.object({
  task_count: z.number(),
  draft: z.number(),
  pending: z.number(),
  in_progress: z.number(),
  done: z.number(),
  blocked: z.number(),
  progress: z.number(),
});
const progress = z.object({
  completed: z.number(),
  total: z.number(),
  percentage: z.number(),
  current_leaf: z.string().nullable(),
  current_failure: z.string().nullable(),
  run_state: z.string().nullable(),
  latest_event: z.string().nullable(),
});
const task = z.object({
  id: z.string(),
  project_code: z.string(),
  group_code: z.string(),
  number: z.string(),
  title: z.string(),
  contract_id: z.string(),
  readme: z.string(),
  metadata: z.object({
    schema: z.literal("task-meta/v1"),
    status: statusSchema,
    dependencies: z.array(z.string()),
  }),
  dependents: z.array(z.string()),
  progress,
  eligibility,
  readiness: eligibility,
  latest_run: z
    .object({
      execution_id: z.string(),
      state: z.string(),
      failure: z.string().nullable(),
      started: z.string(),
      finished: z.string().nullable(),
    })
    .nullable(),
});
const group = z.object({
  id: z.string(),
  project_code: z.string(),
  code: z.string(),
  name: z.string(),
  readme: z.string(),
  tasks: z.array(z.string()),
  summary,
  eligibility,
});
const project = z.object({
  code: z.string(),
  name: z.string(),
  readme: z.string(),
  groups: z.array(z.string()),
  summary,
  eligibility,
});
export const catalogSchema = z.object({
  projects: z.record(z.string(), project),
  groups: z.record(z.string(), group),
  tasks: z.record(z.string(), task),
  summary,
  execution: z.object({
    available: z.boolean(),
    reason: z.string().nullable(),
  }),
  topological_order: z.array(z.string()),
});
export type Catalog = z.infer<typeof catalogSchema>;
export type Task = z.infer<typeof task>;
export type Summary = z.infer<typeof summary>;
export type Eligibility = z.infer<typeof eligibility>;
export type Status = z.infer<typeof statusSchema>;
export type Scope = {
  readonly kind: "task" | "group" | "project";
  readonly id: string;
};
const errorSchema = z.object({
  error: z.object({ code: z.string(), message: z.string() }),
});
async function responseData(response: Response): Promise<unknown> {
  const data: unknown = await response.json();
  if (!response.ok) {
    const parsed = errorSchema.safeParse(data);
    throw new Error(
      parsed.success
        ? parsed.data.error.message
        : `Request failed (${response.status})`,
    );
  }
  return data;
}
export async function fetchCatalog(signal: AbortSignal): Promise<Catalog> {
  const data = await responseData(
    await fetch("/api/catalog", {
      signal,
      headers: { Accept: "application/json" },
    }),
  );
  const parsed = catalogSchema.safeParse(data);
  if (!parsed.success)
    throw new Error(
      "The backend returned an incompatible catalog. Check server and frontend versions.",
    );
  return parsed.data;
}
export async function runScope(
  scope: Scope,
  signal: AbortSignal,
): Promise<void> {
  await responseData(
    await fetch(
      `/api/run/${scope.kind}/${scope.id.split("/").map(encodeURIComponent).join("/")}`,
      {
        method: "POST",
        signal,
        headers: { Accept: "application/json", "X-Task-Monitor": "1" },
      },
    ),
  );
}
export function taskHref(
  task: Pick<Task, "project_code" | "group_code" | "number">,
) {
  return `/projects/${encodeURIComponent(task.project_code)}/groups/${encodeURIComponent(task.group_code)}/tasks/${encodeURIComponent(task.number)}`;
}
export function excerpt(markdown: string) {
  const text = markdown
    .replace(/^#.*$/gm, "")
    .replace(/[*_`[\]]/g, "")
    .trim();
  if (text.length <= 180) return text;
  const clipped = text.slice(0, 177);
  const boundary = clipped.lastIndexOf(" ");
  return `${boundary > 120 ? clipped.slice(0, boundary) : clipped}…`;
}
export function quantity(count: number, noun: string) {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}
