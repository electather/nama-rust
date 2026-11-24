import { createFileRoute, redirect } from "@tanstack/react-router";

import { SignInLinkSent } from "@/features/auth/components/sign-in-link-sent";

export const Route = createFileRoute("/_signed-out/_hub/sign-in-link-sent")({
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
  return <SignInLinkSent email={email} />;
}
