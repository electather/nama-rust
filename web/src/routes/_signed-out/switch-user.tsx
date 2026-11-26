import { createFileRoute, redirect } from "@tanstack/react-router";

import { redirectSearch } from "@/features/auth/lib/redirect-search";
import { whoIsSignedIn } from "@/shared/hooks/access";

// The User picker and its Switch PIN step: they share `redirect`, where switching goes.
export const Route = createFileRoute("/_signed-out/switch-user")({
  validateSearch: redirectSearch,
  beforeLoad: async ({ context }) => {
    const signedIn = await whoIsSignedIn(context);
    if (!signedIn) {
      throw redirect({ to: "/sign-in" });
    }
    return { signedIn };
  },
});
