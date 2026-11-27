import { createFileRoute, redirect } from "@tanstack/react-router";

import { SignInLinkSent } from "@/features/auth/components/sign-in-link-sent";
import { redirectSearch } from "@/features/auth/lib/redirect-search";

// `redirect` is where signing in with a password instead goes, kept from `/send-sign-in-link`.
export const Route = createFileRoute("/_signed-out/_hub/sign-in-link-sent")({
  validateSearch: redirectSearch,
  beforeLoad: ({ location }) => {
    const email = location.state.signInLinkSentTo;
    if (email === undefined) {
      throw redirect({ to: "/send-sign-in-link", replace: true });
    }
    return { email };
  },
  component: SignInLinkSentRoute,
});

function SignInLinkSentRoute() {
  const { email } = Route.useRouteContext();
  const { redirect: goesTo } = Route.useSearch();
  return <SignInLinkSent email={email} redirect={goesTo} />;
}
