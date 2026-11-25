import { createFileRoute } from "@tanstack/react-router";

import { ServerLinkApproval } from "@/features/auth/components/server-link-approval";

// The Hub's page a Server sends its Admin to, to Link it by redirect
// (`proto/nama/hub/v1/link.proto`, `ApproveServerLink`). `_member` signs the Account in first.
export const Route = createFileRoute("/_member/_hub/link-server/$code")({
  component: ServerLinkApprovalRoute,
});

function ServerLinkApprovalRoute() {
  const { code } = Route.useParams();
  return <ServerLinkApproval code={code} />;
}
