import { createFileRoute, redirect } from "@tanstack/react-router";

import { Approval } from "@/features/auth/components/approval";

export const Route = createFileRoute("/_member/code/approve")({
  beforeLoad: ({ location }) => {
    const { lookedUp } = location.state;
    if (lookedUp === undefined) {
      throw redirect({ to: "/code", replace: true });
    }
    return { lookedUp };
  },
  component: ApproveRoute,
});

function ApproveRoute() {
  const { lookedUp } = Route.useRouteContext();
  return <Approval code={lookedUp.code} deviceName={lookedUp.deviceName} />;
}
