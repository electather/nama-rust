import { createRootRouteWithContext } from "@tanstack/react-router";

import { RootLayout } from "@/app/root-layout";
import { NotFound } from "@/app/route-error/not-found";
import type { RouterContext } from "@/shared/hooks/access";
import type { ServerAt } from "@/shared/lib/server";

// `index.html` holds the document: its head, and the element the app renders into.
export const Route = createRootRouteWithContext<RouterContext & { serverAt: ServerAt }>()({
  notFoundComponent: NotFound,
  component: RootLayout,
});
