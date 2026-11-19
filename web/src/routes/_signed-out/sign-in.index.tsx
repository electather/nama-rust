import { createFileRoute } from "@tanstack/react-router";

import { SignIn } from "@/features/auth/components/sign-in";

export const Route = createFileRoute("/_signed-out/sign-in/")({
  component: SignInRoute,
});

function SignInRoute() {
  const { redirect } = Route.useSearch();
  return <SignIn redirect={redirect} />;
}
