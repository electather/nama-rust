import { HubLayout } from "@hub/app/hub-layout";
import { NotFound } from "@hub/app/not-found";
import type { RouterContext } from "@hub/app/router-context";
import { createRootRouteWithContext, Outlet } from "@tanstack/react-router";

export const Route = createRootRouteWithContext<RouterContext>()({
  notFoundComponent: NotFound,
  component: () => (
    <HubLayout>
      <Outlet />
    </HubLayout>
  ),
});
