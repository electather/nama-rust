import { createFileRoute, redirect } from "@tanstack/react-router";

import { redirectSearch } from "@/features/auth/lib/redirect-search";
import { hubInfo, isSetupRequired, skipsSignIn } from "@/shared/hooks/access";

// Every way to sign in: its pages share `redirect`, where signing in goes.
export const Route = createFileRoute("/_signed-out/sign-in")({
  validateSearch: redirectSearch,
  beforeLoad: async ({ context, search }) => {
    if (await isSetupRequired(context)) {
      throw redirect({ to: "/setup" });
    }
    // Going back to a sign-in page once signed in goes on to where signing in went.
    if (await skipsSignIn(context, search.redirect)) {
      throw redirect({ href: search.redirect ?? "/", replace: true });
    }
    // At the Hub, an Account signs in; at a Server, a User (ADR 0105).
    return { hub: await hubInfo(context) };
  },
});
