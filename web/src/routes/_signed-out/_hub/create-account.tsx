import { createFileRoute } from "@tanstack/react-router";

import { CreateAccount } from "@/features/auth/components/create-account";
import { pathOnThisServer } from "@/features/auth/lib/path-on-this-server";

// `redirect` is where signing in goes once the Account is created, as on `/sign-in`.
export const Route = createFileRoute("/_signed-out/_hub/create-account")({
  validateSearch: (search): { redirect?: string | undefined } => ({
    redirect: pathOnThisServer(search["redirect"]),
  }),
  component: CreateAccountRoute,
});

function CreateAccountRoute() {
  const { redirect } = Route.useSearch();
  return <CreateAccount redirect={redirect} />;
}
