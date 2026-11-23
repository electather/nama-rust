import { Outlet, createRootRouteWithContext } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import type { RouterContext } from "@/shared/hooks/access";

// `index.html` holds the document: its head, and the element the app renders into.
export const Route = createRootRouteWithContext<RouterContext>()({
  notFoundComponent: NotFound,
  component: Root,
});

function Root() {
  return (
    <>
      {/* The tab's name while no page names it, such as while one loads. */}
      <title>Nama</title>
      <Outlet />
    </>
  );
}
