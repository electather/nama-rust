import { createFileRoute } from "@tanstack/react-router";

import { FinishAccountSignIn } from "@/features/auth/components/finish-account-sign-in";

// Where the Hub sends a browser back to once an Account approved signing in, adding the one-time
// `code` that finishes it.
export const Route = createFileRoute("/_signed-out/sign-in/_server/account")({
  validateSearch: (search): { code?: string | undefined } => ({
    code: typeof search["code"] === "string" ? search["code"] : undefined,
  }),
  component: AccountSignInRoute,
});

function AccountSignInRoute() {
  const { redirect, code } = Route.useSearch();
  return <FinishAccountSignIn redirect={redirect} code={code ?? ""} />;
}
