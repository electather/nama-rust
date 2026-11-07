import { createFileRoute } from "@tanstack/react-router";

import { NotFound } from "@/components/route-error";

// Any other address needs a session too, so signing in comes before "Page not found".
export const Route = createFileRoute("/_member/$")({ component: NotFound });
