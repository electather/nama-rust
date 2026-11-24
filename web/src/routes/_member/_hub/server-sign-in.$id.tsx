import { createFileRoute } from "@tanstack/react-router";

import { ServerSignIn } from "@/features/auth/components/server-sign-in";
import { ServerSignInFailed } from "@/features/auth/components/server-sign-in-failed";
import { ServerSignInRefused } from "@/features/auth/components/server-sign-in-refused";
import { lookUpServerSignIn } from "@/features/auth/data/look-up-server-sign-in";
import { NO_USER_ON_SERVER } from "@/features/auth/lib/hub-errors";

// The Hub's page a Server sends someone signing in with their Account to
// (`proto/nama/hub/v1/link.proto`, `StartServerSignIn`). `_member` signs the Account in first.
export const Route = createFileRoute("/_member/_hub/server-sign-in/$id")({
  loader: ({ context, params }) => lookUpServerSignIn(context.transport, params.id),
  component: ServerSignInRoute,
  // Not the app's own error page, which blames the Server.
  errorComponent: ServerSignInFailed,
});

function ServerSignInRoute() {
  const { id } = Route.useParams();
  const { serverName, userName } = Route.useLoaderData();
  // The Hub names no User when the Account has none there, which can't sign in to it.
  return userName === "" ? (
    <ServerSignInRefused
      heading={`Sign in to ${serverName}`}
      errors={{ form: NO_USER_ON_SERVER }}
    />
  ) : (
    <ServerSignIn id={id} serverName={serverName} userName={userName} />
  );
}
