import { createFileRoute } from "@tanstack/react-router";
import { Dashboard } from "../components/views";
export const Route = createFileRoute("/")({ component: Dashboard });
