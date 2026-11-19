import { createFileRoute, redirect } from "@tanstack/react-router";

import { Approved } from "@/features/auth/components/approved";

export const Route = createFileRoute("/_member/code/done")({
  beforeLoad: ({ location }) => {
    const deviceName = location.state.signedInDevice;
    if (deviceName === undefined) {
      throw redirect({ to: "/code", replace: true });
    }
    return { deviceName };
  },
  component: ApprovedRoute,
});

function ApprovedRoute() {
  const { deviceName } = Route.useRouteContext();
  return <Approved deviceName={deviceName} />;
}
