import { createFileRoute } from "@tanstack/react-router";

import { AcceptInvite } from "@/features/invites/components/accept-invite";

export const Route = createFileRoute("/_signed-out/_server/invite/$token")({
  component: AcceptInviteRoute,
});

function AcceptInviteRoute() {
  const { token } = Route.useParams();
  return <AcceptInvite token={token} />;
}
