import { createFileRoute, redirect } from "@tanstack/react-router";

import { SignIn } from "@/features/auth/components/sign-in";
import { pathOnThisServer } from "@/features/auth/lib/path-on-this-server";
import { isSetupRequired } from "@/shared/hooks/access";

export const Route = createFileRoute("/sign-in")({
  // The router merges this over the raw search, so a refused `redirect` has to be overwritten.
  validateSearch: (search): { redirect?: string | undefined } => ({
    redirect: pathOnThisServer(search["redirect"]),
  }),
  beforeLoad: async ({ context }) => {
    if (await isSetupRequired(context)) {
      throw redirect({ to: "/setup" });
    }
  },
  component: SignInRoute,
});

function SignInRoute() {
  const { redirect: back } = Route.useSearch();
  return <SignIn redirect={back} />;
}
