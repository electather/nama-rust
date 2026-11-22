import { SignInPage } from "@hub/features/account/components/sign-in-page";
import { hubPath } from "@hub/features/account/lib/hub-path";
import { createFileRoute } from "@tanstack/react-router";

/** Signing in. `?continue=` names the Hub path to go on to, such as a Server's sign-in. */
export const Route = createFileRoute("/sign-in")({
  validateSearch: (search): { continue?: string | undefined } => ({
    continue: hubPath(search["continue"]),
  }),
  component: SignInRoute,
});

function SignInRoute() {
  const { continue: continueTo } = Route.useSearch();
  return <SignInPage continueTo={continueTo ?? "/"} />;
}
