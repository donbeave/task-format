import { Link, Outlet } from "@tanstack/react-router";
import { useCatalog } from "../lib/catalog-context";
export function Shell() {
  const { state, refresh } = useCatalog();
  const projects =
    state.status === "ready" ? Object.values(state.catalog.projects) : [];
  const navigation = (
    <nav aria-label="Primary navigation">
      <Link className="nav-link" to="/" activeOptions={{ exact: true }}>
        Overview
      </Link>
      <Link className="nav-link" to="/tasks/my">
        Tasks / My tasks
      </Link>
      <p className="sidebar-label">Projects</p>
      {projects.map((project) => (
        <Link
          className="nav-link"
          key={project.code}
          to="/projects/$projectCode"
          params={{ projectCode: project.code }}
        >
          {project.name}
        </Link>
      ))}
    </nav>
  );
  return (
    <>
      <a className="skip" href="#main-content">
        Skip to content
      </a>
      <div className="shell">
        <div className="rail">
          <a className="brand" href="/" aria-label="Task monitor home">
            t.
          </a>
          <Link
            className="rail-link"
            to="/"
            activeOptions={{ exact: true }}
            aria-label="Overview"
          >
            ▦
          </Link>
          <Link
            className="rail-link"
            to="/tasks/my"
            aria-label="Dependency tasks"
          >
            ⤳
          </Link>
        </div>
        <aside className="sidebar">
          <h2>Task monitor</h2>
          <p className="muted">Project workspace</p>
          <div className="section">{navigation}</div>
        </aside>
        <div className="surface">
          <div className="topbar">
            <span>Workspace / Progress</span>
            <span
              className={`live ${state.status === "ready" && !state.refreshError ? "" : "offline"}`}
            >
              {state.status === "ready" && !state.refreshError
                ? "Auto-refresh · 3s"
                : "Connecting to backend"}
            </span>
          </div>
          <details className="mobile-nav">
            <summary>Workspace navigation</summary>
            {navigation}
          </details>
          <main className="content" id="main-content" tabIndex={-1}>
            {state.status === "loading" ? (
              <div className="loading" role="status" aria-busy="true">
                Loading projects and task progress…
              </div>
            ) : state.status === "error" ? (
              <div className="error" role="alert">
                <h1>Catalog unavailable</h1>
                <p>{state.message}</p>
                <button type="button" className="refresh" onClick={refresh}>
                  Retry
                </button>
              </div>
            ) : (
              <>
                {state.refreshError && (
                  <div className="error" role="alert">
                    Refresh failed: {state.refreshError} Displaying last known
                    data.{" "}
                    <button type="button" className="refresh" onClick={refresh}>
                      Retry
                    </button>
                  </div>
                )}
                <Outlet />
              </>
            )}
          </main>
        </div>
      </div>
    </>
  );
}
