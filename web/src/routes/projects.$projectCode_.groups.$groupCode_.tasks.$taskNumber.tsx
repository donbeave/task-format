import { createFileRoute } from "@tanstack/react-router";
import { TaskView } from "../components/views";
export const Route = createFileRoute(
  "/projects/$projectCode_/groups/$groupCode_/tasks/$taskNumber",
)({ component: Page });
function Page() {
  return <TaskView {...Route.useParams()} />;
}
