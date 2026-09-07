import { createFileRoute } from "@tanstack/react-router";
import { GroupView } from "../components/views";
export const Route = createFileRoute(
  "/projects/$projectCode_/groups/$groupCode",
)({ component: Page });
function Page() {
  return <GroupView {...Route.useParams()} />;
}
