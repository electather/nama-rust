import { createFileRoute } from "@tanstack/react-router";

import { CreateAccount } from "@/features/auth/components/create-account";
import { redirectSearch } from "@/features/auth/lib/redirect-search";

// `redirect` is where signing in goes once the Account is created, as on `/sign-in`.
export const Route = createFileRoute("/_signed-out/_hub/create-account")({
  validateSearch: redirectSearch,
  component: CreateAccountRoute,
});

function CreateAccountRoute() {
  const { redirect } = Route.useSearch();
  return <CreateAccount redirect={redirect} />;
}
