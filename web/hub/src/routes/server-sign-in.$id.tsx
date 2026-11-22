import { signedInAccount } from "@hub/features/account/data/account";
import { ServerSignInPage } from "@hub/features/links/components/server-sign-in-page";
import { createFileRoute, redirect } from "@tanstack/react-router";

/** Where a Server sends someone signing in to it with their Account. */
export const Route = createFileRoute("/server-sign-in/$id")({
  beforeLoad: async ({ context, location }) => {
    if (!(await signedInAccount(context))) {
      throw redirect({ to: "/sign-in", search: { continue: location.pathname } });
    }
  },
  component: ServerSignInRoute,
});

function ServerSignInRoute() {
  const { id } = Route.useParams();
  return <ServerSignInPage id={id} />;
}
