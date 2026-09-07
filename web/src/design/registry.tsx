import type { ReactElement } from "react";
import { CatalogError, CatalogLoading } from "../components/shell";
import {
  DashboardScreen,
  GroupScreen,
  MyTasksScreen,
  ProjectScreen,
  TaskScreen,
} from "../components/views";
import type { Catalog } from "../lib/api";
import {
  designCatalog,
  designError,
  emptyDesignCatalog,
  emptyWorkspaceCatalog,
} from "./fixtures/catalog";

export const designStates = ["default", "empty", "loading", "error"] as const;
export type DesignState = (typeof designStates)[number];
export interface ScreenEntry {
  readonly id: string;
  readonly name: string;
  readonly states: typeof designStates;
  readonly render: (catalog: Catalog) => ReactElement;
}
export const registry: readonly ScreenEntry[] = [
  {
    id: "dashboard",
    name: "Overview",
    states: designStates,
    render: (catalog) => <DashboardScreen catalog={catalog} />,
  },
  {
    id: "project",
    name: "Project",
    states: designStates,
    render: (catalog) => <ProjectScreen catalog={catalog} code="atlas" />,
  },
  {
    id: "group",
    name: "Group",
    states: designStates,
    render: (catalog) => (
      <GroupScreen
        catalog={catalog}
        projectCode="atlas"
        groupCode="interface"
      />
    ),
  },
  {
    id: "task",
    name: "Task",
    states: designStates,
    render: (catalog) => (
      <TaskScreen
        catalog={catalog}
        projectCode="atlas"
        groupCode="interface"
        taskNumber="002"
      />
    ),
  },
  {
    id: "my-tasks",
    name: "My tasks",
    states: designStates,
    render: (catalog) => <MyTasksScreen catalog={catalog} />,
  },
];
export function findDesign(
  screen: string,
  state: string,
): { readonly screen: ScreenEntry; readonly state: DesignState } | undefined {
  const entry = registry.find((candidate) => candidate.id === screen);
  const matchedState = designStates.find((candidate) => candidate === state);
  return entry && matchedState
    ? { screen: entry, state: matchedState }
    : undefined;
}
export function DesignPreview({
  screen,
  state,
}: {
  readonly screen: ScreenEntry;
  readonly state: DesignState;
}): ReactElement {
  switch (state) {
    case "loading":
      return <CatalogLoading />;
    case "error":
      return <CatalogError message={designError} onRetry={() => undefined} />;
    case "empty":
      return screen.render(
        screen.id === "dashboard" || screen.id === "my-tasks"
          ? emptyWorkspaceCatalog
          : emptyDesignCatalog,
      );
    case "default":
      return screen.render(designCatalog);
  }
}
