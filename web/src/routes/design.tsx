import { createFileRoute, notFound } from "@tanstack/react-router";
import { Shell } from "../components/shell";
import { designCatalog } from "../design/fixtures/catalog";
import { CatalogSnapshotProvider } from "../lib/catalog-context";

export const Route = createFileRoute("/design")({
  beforeLoad: () => {
    if (!import.meta.env.DEV && import.meta.env.VITE_DESIGN_ROUTES !== "1")
      throw notFound();
  },
  component: () => (
    <CatalogSnapshotProvider catalog={designCatalog}>
      <Shell preview />
    </CatalogSnapshotProvider>
  ),
});
