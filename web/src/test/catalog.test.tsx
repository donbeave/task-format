import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { RunAction } from "../components/monitor";
import { CatalogProvider, useCatalog } from "../lib/catalog-context";
import { fixture } from "./fixture";

afterEach(() => vi.unstubAllGlobals());
function Probe() {
  const { state, refresh } = useCatalog();
  return (
    <>
      <button type="button" onClick={refresh}>
        Refresh
      </button>
      {state.status === "loading" ? (
        <p role="status">Loading</p>
      ) : state.status === "error" ? (
        <p role="alert">{state.message}</p>
      ) : (
        <>
          <p>{state.catalog.projects.jackin.name}</p>
          {state.refreshError && <p role="alert">{state.refreshError}</p>}
          <RunAction
            scope={{ kind: "project", id: "jackin" }}
            eligibility={state.catalog.projects.jackin.eligibility}
            label="Run project"
          />
        </>
      )}
    </>
  );
}
it("shows loading, surfaces initial network failure, and recovers on retry", async () => {
  const fetch = vi
    .fn()
    .mockRejectedValueOnce(new Error("Backend offline"))
    .mockImplementation(async () => new Response(JSON.stringify(fixture())));
  vi.stubGlobal("fetch", fetch);
  render(
    <CatalogProvider>
      <Probe />
    </CatalogProvider>,
  );
  expect(screen.getByRole("status")).toHaveTextContent("Loading");
  expect(await screen.findByRole("alert")).toHaveTextContent("Backend offline");
  fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
  expect(await screen.findByText("Jackin")).toBeVisible();
  expect(screen.queryByRole("alert")).toBeNull();
});
it("keeps prior snapshot visibly stale and disables execution after refresh failure", async () => {
  const fetch = vi
    .fn()
    .mockResolvedValueOnce(new Response(JSON.stringify(fixture())))
    .mockRejectedValue(new Error("Refresh unavailable"));
  vi.stubGlobal("fetch", fetch);
  render(
    <CatalogProvider>
      <Probe />
    </CatalogProvider>,
  );
  expect(await screen.findByText("Jackin")).toBeVisible();
  expect(screen.getByRole("button", { name: "Run project" })).toBeEnabled();
  fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Refresh unavailable",
  );
  expect(screen.getByText("Jackin")).toBeVisible();
  expect(screen.getByRole("button", { name: "Run project" })).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Run project" }),
  ).toHaveAccessibleDescription(
    "Execution disabled until fresh catalog data is available.",
  );
});
it("cancels the owned request on unmount", async () => {
  const signals: AbortSignal[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn((_url: string, init: RequestInit) => {
      if (init.signal) signals.push(init.signal);
      return new Promise<Response>(() => {});
    }),
  );
  const { unmount } = render(
    <CatalogProvider>
      <Probe />
    </CatalogProvider>,
  );
  await waitFor(() => expect(signals).toHaveLength(1));
  unmount();
  expect(signals[0].aborted).toBe(true);
});
