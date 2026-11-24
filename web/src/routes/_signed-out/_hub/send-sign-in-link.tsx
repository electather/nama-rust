import { createFileRoute } from "@tanstack/react-router";

import { SendSignInLink } from "@/features/auth/components/send-sign-in-link";
import { pathOnThisServer } from "@/features/auth/lib/path-on-this-server";

// `redirect` is where the link's sign-in goes, as on `/sign-in`.
export const Route = createFileRoute("/_signed-out/_hub/send-sign-in-link")({
  validateSearch: (search): { redirect?: string | undefined } => ({
    redirect: pathOnThisServer(search["redirect"]),
  }),
  component: SendSignInLinkRoute,
});

function SendSignInLinkRoute() {
  const { redirect } = Route.useSearch();
  return <SendSignInLink redirect={redirect} />;
}
