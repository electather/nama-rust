import { createFileRoute, redirect } from "@tanstack/react-router";

import { pathOnThisServer } from "@/features/auth/lib/path-on-this-server";
import { hubInfo, isSetupRequired, isSignedInAlready } from "@/shared/hooks/access";

// Every way to sign in: its pages share `redirect`, where signing in goes.
export const Route = createFileRoute("/_signed-out/sign-in")({
  // The router merges this over the raw search, so a refused `redirect` has to be overwritten.
  validateSearch: (search): { redirect?: string | undefined } => ({
    redirect: pathOnThisServer(search["redirect"]),
  }),
  beforeLoad: async ({ context, search }) => {
    if (await isSetupRequired(context)) {
      throw redirect({ to: "/setup" });
    }
    // Going back to a sign-in page once signed in goes on to where signing in went.
    if (await isSignedInAlready(context, search.redirect)) {
      throw redirect({ href: search.redirect ?? "/", replace: true });
    }
    // At the Hub, an Account signs in; at a Server, a User (ADR 0105).
    return { hub: await hubInfo(context) };
  },
});
