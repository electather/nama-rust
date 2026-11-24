import { createFileRoute } from "@tanstack/react-router";

import { HubSignIn } from "@/features/auth/components/hub-sign-in";
import { SignIn } from "@/features/auth/components/sign-in";

export const Route = createFileRoute("/_signed-out/sign-in/")({
  component: SignInRoute,
});

function SignInRoute() {
  const { redirect } = Route.useSearch();
  const { hub } = Route.useRouteContext();
  return hub ? (
    <HubSignIn redirect={redirect} sendsSignInLinks={hub.signInLinks} />
  ) : (
    <SignIn redirect={redirect} />
  );
}
