import { createFileRoute } from "@tanstack/react-router";

import { ExpiredSignInLink } from "@/features/auth/components/expired-sign-in-link";
import { SignInLink } from "@/features/auth/components/sign-in-link";
import { SignInLinkFailed } from "@/features/auth/components/sign-in-link-failed";
import { lookUpSignInLink } from "@/features/auth/data/look-up-sign-in-link";
import { signedInAccount } from "@/shared/hooks/access";

// The page a Sign-in Link opens (`crates/nama-hub/src/accounts/sign_in_links.rs`). Its token is
// the fragment, which never reaches a server's logs.
export const Route = createFileRoute("/_signed-out/_hub/sign-in-link")({
  loader: async ({ context, location }) => {
    const [link, signedIn] = await Promise.all([
      lookUpSignInLink(context.transport, location.hash),
      signedInAccount(context),
    ]);
    // Kept with the page: signing in changes the location before the page goes away.
    return { token: location.hash, link, signedIn };
  },
  component: SignInLinkRoute,
  // Not the app's own error page, which blames the Server.
  errorComponent: SignInLinkFailed,
});

function SignInLinkRoute() {
  const { token, link, signedIn } = Route.useLoaderData();
  return link?.account ? (
    <SignInLink
      token={token}
      account={link.account}
      emailProven={link.emailProven}
      signedIn={signedIn}
    />
  ) : (
    <ExpiredSignInLink />
  );
}
