import { createFileRoute, Link, notFound } from "@tanstack/react-router";
import { DesignPreview, findDesign } from "../design/registry";

export const Route = createFileRoute("/design/$screen/$state")({
  beforeLoad: ({ params }) => {
    if (!findDesign(params.screen, params.state)) throw notFound();
  },
  component: DesignRoute,
});
function DesignRoute() {
  const { screen, state } = Route.useParams();
  const entry = findDesign(screen, state);
  if (!entry) throw new Error("Validated design route is missing its fixture");
  return (
    <div className="page-stack">
      <nav aria-label="Design gallery">
        <Link to="/design">Design gallery</Link>
        <span className="muted">
          {" "}
          / {entry.screen.name} / {entry.state} · Synthetic preview
        </span>
      </nav>
      <DesignPreview {...entry} />
    </div>
  );
}
