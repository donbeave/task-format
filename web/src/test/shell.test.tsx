import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { CatalogLoading } from "../components/shell";
import { getRouter } from "../router";
import { fixture } from "./fixture";

afterEach(() => vi.unstubAllGlobals());

async function openWorkspace() {
  const fetch = vi.fn(
    async () =>
      new Response(JSON.stringify(fixture()), {
        headers: { "Content-Type": "application/json" },
      }),
  );
  vi.stubGlobal("fetch", fetch);
  const router = getRouter();
  router.update({ history: createMemoryHistory({ initialEntries: ["/"] }) });
  const rendered = render(<RouterProvider router={router} />);
  await screen.findByRole("heading", { level: 1, name: "Project overview" });
  return { ...rendered, fetch };
}

it("collapses and restores workspace navigation without hiding content", async () => {
  await openWorkspace();
  const sidebar = screen.getByRole("complementary", {
    name: "Workspace navigation",
  });
  fireEvent.click(screen.getByRole("button", { name: "Collapse sidebar" }));
  expect(sidebar).toHaveAttribute("hidden");
  expect(screen.getByRole("main")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Expand sidebar" }));
  expect(sidebar).not.toHaveAttribute("hidden");
});

it("changes theme and refreshes the existing catalog connection", async () => {
  const { container, fetch } = await openWorkspace();
  fireEvent.click(screen.getByRole("button", { name: "Switch to dark theme" }));
  expect(container.querySelector(".shell")).toHaveClass("dark");
  fireEvent.click(
    screen.getByRole("button", { name: "Switch to light theme" }),
  );
  expect(container.querySelector(".shell")).not.toHaveClass("dark");
  fireEvent.click(screen.getByRole("button", { name: "Use dark theme" }));
  expect(container.querySelector(".shell")).toHaveClass("dark");
  fireEvent.click(screen.getByRole("button", { name: "Use light theme" }));
  expect(container.querySelector(".shell")).not.toHaveClass("dark");
  const requests = fetch.mock.calls.length;
  fireEvent.click(screen.getByRole("button", { name: "Refresh catalog" }));
  await waitFor(() =>
    expect(fetch.mock.calls.length).toBeGreaterThan(requests),
  );
});

it("keeps loading accessible while reserving dashboard geometry", () => {
  const { container } = render(<CatalogLoading />);
  expect(screen.getByRole("status")).toHaveTextContent(
    "Loading projects and task progress",
  );
  expect(screen.getByRole("status")).toHaveAttribute("aria-busy", "true");
  expect(container.querySelectorAll(".skeleton-stat")).toHaveLength(6);
  expect(container.querySelectorAll(".skeleton-panel")).toHaveLength(2);
});
