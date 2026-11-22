import { SignInLinkPage } from "@hub/features/account/components/sign-in-link-page";
import { signedInAccount } from "@hub/features/account/data/account";
import { createFileRoute } from "@tanstack/react-router";

/** The page a Sign-in Link opens. Its token is the fragment, which never reaches a server's logs. */
export const Route = createFileRoute("/sign-in-link")({
  beforeLoad: async ({ context, location }) => {
    // Read whether another Account is signed in before the page says so.
    await signedInAccount(context);
    // Kept in the match: signing in changes the location before the page goes away.
    return { token: location.hash };
  },
  component: SignInLinkRoute,
});

function SignInLinkRoute() {
  const token = Route.useRouteContext({ select: (context) => context.token });
  return <SignInLinkPage token={token} />;
}
