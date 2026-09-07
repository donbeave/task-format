import { createFileRoute } from "@tanstack/react-router";
import { MyTasks } from "../components/views";
export const Route = createFileRoute("/tasks/my")({ component: MyTasks });
