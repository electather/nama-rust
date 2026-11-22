import { HubLayout } from "@hub/app/hub-layout";
import type { RouterContext } from "@hub/app/router-context";
import { createRootRouteWithContext, Outlet } from "@tanstack/react-router";

export const Route = createRootRouteWithContext<RouterContext>()({
  component: () => (
    <HubLayout>
      <Outlet />
    </HubLayout>
  ),
});
