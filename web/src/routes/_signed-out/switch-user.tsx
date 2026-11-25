import { createFileRoute, redirect } from "@tanstack/react-router";

import { pathOnThisServer } from "@/features/auth/lib/path-on-this-server";
import { whoIsSignedIn } from "@/shared/hooks/access";

// The User picker and its Switch PIN step: they share `redirect`, where switching goes.
export const Route = createFileRoute("/_signed-out/switch-user")({
  validateSearch: (search): { redirect?: string | undefined } => ({
    redirect: pathOnThisServer(search["redirect"]),
  }),
  beforeLoad: async ({ context }) => {
    const signedIn = await whoIsSignedIn(context);
    if (!signedIn) {
      throw redirect({ to: "/sign-in" });
    }
    return { signedIn };
  },
});
