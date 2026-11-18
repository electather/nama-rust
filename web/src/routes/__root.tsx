import { createRootRouteWithContext } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import type { RouterContext } from "@/lib/access";

// `index.html` holds the document: its head, and the element the app renders into.
export const Route = createRootRouteWithContext<RouterContext>()({
  notFoundComponent: NotFound,
});
