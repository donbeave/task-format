import { createFileRoute, Link } from "@tanstack/react-router";
import { registry } from "../design/registry";

export const Route = createFileRoute("/design/")({ component: DesignIndex });
function DesignIndex() {
  return (
    <section className="page-stack">
      <header className="page-heading">
        <div>
          <p className="eyebrow">Synthetic fixtures · No execution</p>
          <h1>Design gallery</h1>
          <p className="muted">
            Verve v1.1.0-inspired workspace. Draft: awaiting your visual
            approval.
          </p>
        </div>
      </header>
      <p className="muted">
        Review both themes at 1440 × 900, 390 × 844, and 320 × 844. Theme
        control lives in the workspace rail.
      </p>
      <div className="project-grid">
        {registry.map((screen) => (
          <section className="panel" key={screen.id}>
            <header className="panel-heading">
              <h2>{screen.name}</h2>
            </header>
            <ul>
              {screen.states.map((state) => (
                <li key={state}>
                  <Link
                    to="/design/$screen/$state"
                    params={{ screen: screen.id, state }}
                  >
                    {state}
                  </Link>
                </li>
              ))}
            </ul>
          </section>
        ))}
      </div>
    </section>
  );
}
