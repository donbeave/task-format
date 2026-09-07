import { createFileRoute } from "@tanstack/react-router";
import { ProjectView } from "../components/views";
export const Route = createFileRoute("/projects/$projectCode")({
  component: Page,
});
function Page() {
  const { projectCode } = Route.useParams();
  return <ProjectView code={projectCode} />;
}
