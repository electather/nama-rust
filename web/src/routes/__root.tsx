import { createRootRouteWithContext } from "@tanstack/react-router";

import { NotFound } from "@/components/route-error";
import type { RouterContext } from "@/lib/access";

// `index.html` holds the document: its head, and the element the app renders into.
export const Route = createRootRouteWithContext<RouterContext>()({
  notFoundComponent: NotFound,
});
