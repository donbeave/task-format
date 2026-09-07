import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  designCatalog,
  emptyDesignCatalog,
  emptyWorkspaceCatalog,
} from "../design/fixtures/catalog";
import { findDesign, registry } from "../design/registry";
import { catalogSchema } from "../lib/api";
import { getRouter } from "../router";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.unstubAllEnvs();
});
async function openDesign(path: string) {
  const fetch = vi
    .fn()
    .mockRejectedValue(new Error("Backend deliberately unavailable"));
  vi.stubGlobal("fetch", fetch);
  const router = getRouter();
  router.update({ history: createMemoryHistory({ initialEntries: [path] }) });
  render(<RouterProvider router={router} />);
  await waitFor(() => expect(router.state.status).toBe("idle"));
  return { fetch, router };
}

describe("deterministic design gallery", () => {
  it("validates every fixture and denies all execution scopes", () => {
    for (const catalog of [
      designCatalog,
      emptyDesignCatalog,
      emptyWorkspaceCatalog,
    ]) {
      expect(catalogSchema.safeParse(catalog).success).toBe(true);
      expect(catalog.execution.available).toBe(false);
      for (const scope of [
        ...Object.values(catalog.projects),
        ...Object.values(catalog.groups),
        ...Object.values(catalog.tasks),
      ])
        expect(scope.eligibility.allowed).toBe(false);
    }
  });
  it("keeps dependency readiness separate from execution permission", async () => {
    expect(
      Object.values(designCatalog.tasks).some((task) => task.readiness.allowed),
    ).toBe(true);
    const { fetch } = await openDesign("/design/my-tasks/default");
    expect(
      await screen.findByRole("table", {
        name: "Tasks whose dependencies are ready",
      }),
    ).toBeVisible();
    expect(fetch).not.toHaveBeenCalled();
  });
  for (const entry of registry) {
    for (const state of entry.states) {
      it(`renders ${entry.id}/${state} without backend reads or writes`, async () => {
        const { fetch } = await openDesign(`/design/${entry.id}/${state}`);
        expect(
          await screen.findByRole("navigation", { name: "Design gallery" }),
        ).toBeVisible();
        if (state === "loading")
          expect(screen.getByRole("status")).toHaveAttribute(
            "aria-busy",
            "true",
          );
        else if (state === "error") {
          expect(screen.getByRole("alert")).toHaveTextContent(
            "The catalog folder could not be read",
          );
          fireEvent.click(screen.getByRole("button", { name: "Retry" }));
        } else expect(screen.getByRole("heading", { level: 1 })).toBeVisible();
        for (const button of screen.queryAllByRole("button", {
          name: /^Run /,
        })) {
          expect(button).toBeDisabled();
          fireEvent.click(button);
        }
        expect(fetch).not.toHaveBeenCalled();
      });
    }
  }
  it("enumerates all 20 screen-state links without live catalog", async () => {
    const { fetch } = await openDesign("/design");
    expect(
      await screen.findByRole("heading", { name: "Design gallery" }),
    ).toBeVisible();
    const links = screen
      .getAllByRole("link")
      .filter((link) =>
        link.getAttribute("href")?.match(/^\/design\/[^/]+\/[^/]+$/),
      );
    expect(links).toHaveLength(20);
    expect(fetch).not.toHaveBeenCalled();
  });
  it.each(["/design/absent/default", "/design/dashboard/absent"])(
    "rejects unknown fixture route %s",
    async (path) => {
      const { fetch } = await openDesign(path);
      expect(
        await screen.findByRole("heading", { name: "Page not found" }),
      ).toBeVisible();
      expect(
        screen.queryByRole("navigation", { name: "Design gallery" }),
      ).toBeNull();
      expect(fetch).not.toHaveBeenCalled();
    },
  );
  it("rejects inherited object property names as screen/state identifiers", () => {
    expect(findDesign("constructor", "default")).toBeUndefined();
    expect(findDesign("dashboard", "toString")).toBeUndefined();
  });
  it("guards production routes without revealing synthetic fixture data", async () => {
    vi.stubEnv("DEV", false);
    vi.stubEnv("VITE_DESIGN_ROUTES", "0");
    const { fetch } = await openDesign("/design/project/default");
    expect(
      await screen.findByRole("heading", { name: "Page not found" }),
    ).toBeVisible();
    expect(screen.queryByText("Atlas — Interface laboratory")).toBeNull();
    expect(fetch).not.toHaveBeenCalled();
  });
  it("allows explicit design mode against production environment", async () => {
    vi.stubEnv("DEV", false);
    vi.stubEnv("VITE_DESIGN_ROUTES", "1");
    const { fetch } = await openDesign("/design/dashboard/default");
    expect(
      await screen.findByRole("heading", { name: "Project overview" }),
    ).toBeVisible();
    expect(fetch).not.toHaveBeenCalled();
  });
});
