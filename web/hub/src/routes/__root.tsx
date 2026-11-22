import { HubLayout } from "@hub/app/hub-layout";
import type { RouterContext } from "@hub/features/account/data/account";
import { createRootRouteWithContext, Outlet } from "@tanstack/react-router";

export const Route = createRootRouteWithContext<RouterContext>()({
  component: () => (
    <HubLayout>
      <Outlet />
    </HubLayout>
  ),
});
