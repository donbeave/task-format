import { Link, Outlet } from "@tanstack/react-router";
import {
  Activity,
  ChevronRight,
  CircleCheck,
  FolderClosed,
  Layers2,
  LayoutGrid,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  RefreshCw,
  Sun,
} from "lucide-react";
import { type ReactNode, useState } from "react";
import { useCatalog } from "../lib/catalog-context";
import { Button } from "./ui/button";

export function Shell({ preview = false }: { readonly preview?: boolean }) {
  const { state, refresh } = useCatalog();
  return (
    <ShellFrame state={state} refresh={refresh} preview={preview}>
      <Outlet />
    </ShellFrame>
  );
}

export function CatalogLoading() {
  return (
    <div className="catalog-loading" role="status" aria-busy="true">
      <span className="sr-only">Loading projects and task progress…</span>
      <div className="skeleton-page-heading" aria-hidden="true">
        <div className="skeleton skeleton-title" />
        <div className="skeleton skeleton-subtitle" />
      </div>
      <div className="skeleton-stats" aria-hidden="true">
        {["projects", "draft", "pending", "active", "blocked", "completed"].map(
          (key) => (
            <div className="skeleton-stat" key={key}>
              <div className="skeleton skeleton-label" />
              <div className="skeleton skeleton-value" />
            </div>
          ),
        )}
      </div>
      <div className="skeleton-panels" aria-hidden="true">
        {["projects", "activity"].map((key) => (
          <div className="skeleton-panel" key={key}>
            <div className="skeleton skeleton-label" />
            {["first", "second", "third"].map((row) => (
              <div className="skeleton skeleton-row" key={row} />
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

export function CatalogError({
  message,
  onRetry,
}: {
  readonly message: string;
  readonly onRetry: () => void;
}) {
  return (
    <div className="error" role="alert">
      <h1>Catalog unavailable</h1>
      <p>{message}</p>
      <Button variant="outline" size="sm" onClick={onRetry}>
        <RefreshCw aria-hidden="true" />
        Retry
      </Button>
    </div>
  );
}

export function ShellFrame({
  state,
  refresh,
  children,
  preview = false,
}: Readonly<ReturnType<typeof useCatalog>> & {
  readonly children: ReactNode;
  readonly preview?: boolean;
}) {
  const [collapsed, setCollapsed] = useState(false);
  const [dark, setDark] = useState(false);
  const projects =
    state.status === "ready" ? Object.values(state.catalog.projects) : [];
  const connected = state.status === "ready" && !state.refreshError;
  const navigation = (
    <nav aria-label="Primary navigation" className="workspace-navigation">
      <p className="sidebar-label">Workspace</p>
      <Link className="nav-link" to="/" activeOptions={{ exact: true }}>
        <LayoutGrid aria-hidden="true" />
        <span>Overview</span>
      </Link>
      <Link className="nav-link" to="/tasks/my">
        <CircleCheck aria-hidden="true" />
        <span>My tasks</span>
      </Link>
      <div className="sidebar-section-heading">
        <p className="sidebar-label">Projects</p>
        <span className="nav-count">{projects.length}</span>
      </div>
      {projects.map((project) => (
        <Link
          className="nav-link project-nav-link"
          key={project.code}
          to="/projects/$projectCode"
          params={{ projectCode: project.code }}
        >
          <FolderClosed aria-hidden="true" />
          <span>{project.name}</span>
          <ChevronRight className="nav-chevron" aria-hidden="true" />
        </Link>
      ))}
      {state.status === "ready" && projects.length === 0 && (
        <p className="sidebar-empty">No projects discovered.</p>
      )}
    </nav>
  );
  return (
    <div
      className={`shell${collapsed ? " sidebar-collapsed" : ""}${dark ? " dark" : ""}`}
    >
      <a className="skip" href="#main-content">
        Skip to content
      </a>
      <div className="rail">
        <Link className="brand" to="/" aria-label="Task monitor home">
          <Layers2 aria-hidden="true" />
        </Link>
        <nav className="rail-navigation" aria-label="Workspace shortcuts">
          <Link
            className="rail-link"
            to="/"
            activeOptions={{ exact: true }}
            aria-label="Overview"
            title="Overview"
          >
            <LayoutGrid aria-hidden="true" />
          </Link>
          <Link
            className="rail-link"
            to="/tasks/my"
            aria-label="Dependency tasks"
            title="My tasks"
          >
            <CircleCheck aria-hidden="true" />
          </Link>
        </nav>
        <div className="rail-footer">
          <Button
            variant="ghost"
            size="icon"
            className="rail-theme"
            onClick={() => setDark((value) => !value)}
            aria-label={dark ? "Switch to light theme" : "Switch to dark theme"}
            title={dark ? "Switch to light theme" : "Switch to dark theme"}
          >
            {dark ? <Sun aria-hidden="true" /> : <Moon aria-hidden="true" />}
          </Button>
          <span
            className="workspace-avatar"
            role="img"
            aria-label="Local workspace"
          >
            TM
          </span>
        </div>
      </div>
      <header className="topbar">
        <div className="workspace-breadcrumb">
          <Layers2 aria-hidden="true" />
          <span>Workspace</span>
          <span className="breadcrumb-divider">/</span>
          <span className="workspace-app-name">Task monitor</span>
        </div>
        <div className="topbar-actions">
          <Button
            variant="ghost"
            size="icon-sm"
            className="mobile-theme"
            onClick={() => setDark((value) => !value)}
            aria-label={dark ? "Use light theme" : "Use dark theme"}
            title={dark ? "Use light theme" : "Use dark theme"}
          >
            {dark ? <Sun aria-hidden="true" /> : <Moon aria-hidden="true" />}
          </Button>
          <span className={`live${connected || preview ? "" : " offline"}`}>
            <span className="live-dot" />
            {preview
              ? "Design preview"
              : connected
                ? "Auto-refresh · 3s"
                : "Connecting to backend"}
          </span>
          {!preview && (
            <Button
              variant="outline"
              size="sm"
              onClick={refresh}
              aria-label="Refresh catalog"
            >
              <RefreshCw aria-hidden="true" />
              <span className="refresh-label">Refresh</span>
            </Button>
          )}
        </div>
      </header>
      <aside
        className="sidebar"
        aria-label="Workspace navigation"
        hidden={collapsed}
      >
        <div className="sidebar-heading">
          <h2>Task monitor</h2>
          <span className="workspace-label">Local</span>
        </div>
        {navigation}
        <div className="sidebar-footer">
          <Activity aria-hidden="true" />
          <div>
            <span>Filesystem workspace</span>
            <code>projects/</code>
          </div>
        </div>
      </aside>
      <Button
        variant="outline"
        size="icon-xs"
        className="sidebar-toggle"
        onClick={() => setCollapsed((value) => !value)}
        aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        aria-expanded={!collapsed}
      >
        {collapsed ? (
          <PanelLeftOpen aria-hidden="true" />
        ) : (
          <PanelLeftClose aria-hidden="true" />
        )}
      </Button>
      <div className="surface">
        <details className="mobile-nav">
          <summary>Workspace navigation</summary>
          {navigation}
        </details>
        <main className="content" id="main-content" tabIndex={-1}>
          {state.status === "loading" ? (
            <CatalogLoading />
          ) : state.status === "error" ? (
            <CatalogError message={state.message} onRetry={refresh} />
          ) : (
            <>
              {state.refreshError && (
                <div className="error" role="alert">
                  Refresh failed: {state.refreshError} Displaying last known
                  data.{" "}
                  <Button variant="outline" size="sm" onClick={refresh}>
                    Retry
                  </Button>
                </div>
              )}
              {children}
            </>
          )}
        </main>
      </div>
    </div>
  );
}
