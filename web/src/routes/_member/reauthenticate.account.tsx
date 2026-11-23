import { createFileRoute } from "@tanstack/react-router";

import { FinishAccountReauthentication } from "@/features/auth/components/finish-account-reauthentication";
import { pathOnThisServer } from "@/features/auth/lib/path-on-this-server";

// Where the Hub sends a browser back to once an Account approved “Confirm it’s you”, adding the
// one-time `code` that finishes it.
export const Route = createFileRoute("/_member/reauthenticate/account")({
  // The router merges this over the raw search, so a refused `redirect` has to be overwritten.
  validateSearch: (search): { redirect?: string | undefined; code?: string | undefined } => ({
    redirect: pathOnThisServer(search["redirect"]),
    code: typeof search["code"] === "string" ? search["code"] : undefined,
  }),
  component: ReauthenticateWithAccountRoute,
});

function ReauthenticateWithAccountRoute() {
  const { redirect, code } = Route.useSearch();
  return <FinishAccountReauthentication redirect={redirect} code={code ?? ""} />;
}
