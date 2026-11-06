import { createFileRoute } from "@tanstack/react-router";

import { ServerInfo } from "@/components/server-info";

export const Route = createFileRoute("/")({ component: ServerInfo });
