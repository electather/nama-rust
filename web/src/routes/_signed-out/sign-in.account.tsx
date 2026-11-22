import { createFileRoute } from "@tanstack/react-router";

import { FinishAccountSignIn } from "@/features/auth/components/finish-account-sign-in";

// Where the Hub sends a browser back to once an Account approved signing in.
export const Route = createFileRoute("/_signed-out/sign-in/account")({
  component: AccountSignInRoute,
});

function AccountSignInRoute() {
  const { redirect } = Route.useSearch();
  return <FinishAccountSignIn redirect={redirect} />;
}
