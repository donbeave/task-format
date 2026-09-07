import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Readme } from "../components/monitor";
import { dependencyStages } from "../components/views";
import { catalogSchema, runScope } from "../lib/api";
import { getRouter } from "../router";
import { fixture } from "./fixture";

afterEach(() => vi.unstubAllGlobals());
async function open(path: string, catalog = fixture()) {
  const fetch = vi.fn().mockResolvedValue(
    new Response(JSON.stringify(catalog), {
      headers: { "Content-Type": "application/json" },
    }),
  );
  // Each poll consumes its own Response body.
  fetch.mockImplementation(
    async () =>
      new Response(JSON.stringify(catalog), {
        headers: { "Content-Type": "application/json" },
      }),
  );
  vi.stubGlobal("fetch", fetch);
  const router = getRouter();
  router.update({ history: createMemoryHistory({ initialEntries: [path] }) });
  render(<RouterProvider router={router} />);
  await screen.findByRole("heading", { level: 1 });
  return { router, fetch };
}
describe("actual nested routes", () => {
  it("shows an empty catalog without invented projects", async () => {
    const catalog = fixture();
    catalog.projects = {};
    catalog.groups = {};
    catalog.tasks = {};
    catalog.topological_order = [];
    catalog.summary = {
      task_count: 0,
      draft: 0,
      pending: 0,
      in_progress: 0,
      blocked: 0,
      done: 0,
      progress: 0,
    };
    await open("/", catalog);
    expect(screen.getByText(/No projects found/)).toBeVisible();
    expect(screen.getByText("0 tasks across 0 projects")).toBeVisible();
  });
  it("explains why a draft task cannot execute", async () => {
    await open("/projects/jackin/groups/design/tasks/005");
    expect(screen.getByRole("button", { name: "Run task" })).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Run task" }),
    ).toHaveAccessibleDescription("Draft tasks never execute");
  });
  it("dashboard displays filesystem project and aggregate counts", async () => {
    await open("/");
    expect(
      screen.getByRole("heading", { level: 1, name: "Project overview" }),
    ).toBeVisible();
    expect(screen.getByText("5 tasks across 1 project")).toBeVisible();
    expect(
      screen.getByRole("link", { name: /Jackin.*Project description/s }),
    ).toHaveAttribute("href", "/projects/jackin");
  });
  it("project renders README and navigable group", async () => {
    await open("/projects/jackin");
    expect(screen.getByText("Project description.")).not.toBeVisible();
    fireEvent.click(screen.getByText("Project brief"));
    expect(screen.getByText("Project description.")).toBeVisible();
    expect(
      screen.getByRole("link", { name: /Design 5 tasks/ }),
    ).toHaveAttribute("href", "/projects/jackin/groups/design");
    expect(screen.getByRole("button", { name: "Run project" })).toBeEnabled();
  });
  it("group renders all semantic Kanban columns and blocked cards", async () => {
    await open("/projects/jackin/groups/design");
    expect(screen.getByText("Group description.")).not.toBeVisible();
    fireEvent.click(screen.getByText("Group brief"));
    expect(screen.getByText("Group description.")).toBeVisible();
    for (const name of [
      "Draft tasks",
      "Pending tasks",
      "In Progress tasks",
      "Done tasks",
    ])
      expect(screen.getByRole("region", { name })).toBeVisible();
    expect(
      within(
        screen.getByRole("region", { name: "Pending tasks" }),
      ).getAllByText("Blocked"),
    ).toHaveLength(3);
    expect(screen.getByRole("region", { name: "Task Kanban" })).toHaveAttribute(
      "tabindex",
      "0",
    );
  });
  it("task renders contract, dependencies and disabled reason accessibly", async () => {
    await open("/projects/jackin/groups/design/tasks/002");
    expect(screen.getByText("Complete task contract.")).toBeVisible();
    const button = screen.getByRole("button", { name: "Run task" });
    expect(button).toBeDisabled();
    expect(button).toHaveAccessibleDescription("Waiting for jackin/design/001");
    expect(
      screen.getAllByRole("link", { name: "jackin/design/001 · Pending" }),
    ).toHaveLength(1);
    expect(screen.getByText("Next check", { selector: "dd" })).toBeVisible();
  });
  it("My tasks shows ready root and parallel branches without drafts", async () => {
    await open("/tasks/my");
    expect(screen.getByRole("heading", { name: "My tasks" })).toBeVisible();
    expect(
      screen.getByRole("table", { name: "Tasks whose dependencies are ready" }),
    ).toBeVisible();
    expect(
      screen.getByRole("region", { name: "Ready tasks table" }),
    ).toHaveAttribute("tabindex", "0");
    expect(screen.getByText("Stage 3")).toBeVisible();
    expect(screen.getByText("2 parallel branches")).toBeVisible();
    expect(screen.queryByText("Task 005")).toBeNull();
    expect(screen.getAllByText("May unlock")).toHaveLength(4);
  });
  it("does not mislabel dependency-ready task when execution unavailable", async () => {
    const catalog = fixture();
    catalog.tasks["jackin/design/001"].eligibility = {
      allowed: false,
      reason: "Execution not configured",
    };
    await open("/projects/jackin/groups/design", catalog);
    const pending = screen.getByRole("region", { name: "Pending tasks" });
    expect(within(pending).getAllByText("Blocked")).toHaveLength(3);
    expect(within(pending).getByText("Ready now")).toBeVisible();
  });
  it("shows missing record without silently substituting another project", async () => {
    await open("/projects/absent");
    expect(
      screen.getByRole("heading", { name: "This record is unavailable" }),
    ).toBeVisible();
  });
  it("performs run and sends required same-origin protection header", async () => {
    const { fetch } = await open("/projects/jackin/groups/design/tasks/001");
    fireEvent.click(screen.getByRole("button", { name: "Run task" }));
    await waitFor(() =>
      expect(fetch).toHaveBeenCalledWith(
        "/api/run/task/jackin/design/001",
        expect.objectContaining({
          method: "POST",
          headers: expect.objectContaining({ "X-Task-Monitor": "1" }),
        }),
      ),
    );
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Execution accepted",
    );
  });
});
describe("boundaries and graph", () => {
  it("makes code scroll regions keyboard focusable", () => {
    render(<Readme>{"```gherkin\nGiven a long task contract\n```"}</Readme>);
    expect(
      screen.getByRole("region", { name: "Task code block" }),
    ).toHaveAttribute("tabindex", "0");
  });
  it("places diamond join after both branches and ignores draft dependency", () => {
    const catalog = fixture();
    catalog.tasks["jackin/design/001"].metadata.dependencies = [
      "jackin/design/005",
    ];
    expect(
      dependencyStages(catalog).map((stage) =>
        stage.map((task) => task.number),
      ),
    ).toEqual([["001"], ["002", "003"], ["004"]]);
  });
  it("rejects incompatible API shape and invalid persisted status", () => {
    expect(catalogSchema.safeParse({}).success).toBe(false);
    const catalog = fixture();
    expect(
      catalogSchema.safeParse({
        ...catalog,
        tasks: {
          bad: {
            ...catalog.tasks["jackin/design/001"],
            metadata: {
              schema: "task-meta/v1",
              status: "blocked",
              dependencies: [],
            },
          },
        },
      }).success,
    ).toBe(false);
  });
  it("strips unknown response properties for forward compatibility", () => {
    expect(
      catalogSchema.parse({ ...fixture(), future: "value" }),
    ).not.toHaveProperty("future");
  });
  it("renders safe markdown without active HTML or image network requests", () => {
    const { container } = render(
      <Readme>
        {
          "# Contract\n\n<script>alert(1)</script>\n\n[unsafe](javascript:alert(1))\n\n![remote](https://example.com/track.png)"
        }
      </Readme>,
    );
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("a")).toBeNull();
  });
  it("renders checklist states with accessible names instead of unlabeled controls", () => {
    render(<Readme>{"- [x] First check\n- [ ] Next check"}</Readme>);
    expect(
      screen.getByRole("img", { name: "Completed checklist item" }),
    ).toBeVisible();
    expect(
      screen.getByRole("img", { name: "Incomplete checklist item" }),
    ).toBeVisible();
    expect(screen.queryByRole("checkbox")).toBeNull();
  });
  it("surfaces typed backend rejection", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            error: { code: "conflict", message: "Task already running" },
          }),
          { status: 409 },
        ),
      ),
    );
    await expect(
      runScope(
        { kind: "task", id: "jackin/design/001" },
        new AbortController().signal,
      ),
    ).rejects.toThrow("Task already running");
  });
});
