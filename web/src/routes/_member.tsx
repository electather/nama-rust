import { createFileRoute, redirect } from "@tanstack/react-router";

import { MemberLayout } from "@/app/member-layout";
import { isSetupRequired, whoIsSignedIn } from "@/lib/access";
import { recordDiagnosticsIfExported } from "@/lib/diagnostics";

/** Pages for signed-in Users. */
export const Route = createFileRoute("/_member")({
  beforeLoad: async ({ context, location }) => {
    if (await isSetupRequired(context)) {
      throw redirect({ to: "/setup" });
    }
    const signedIn = await whoIsSignedIn(context);
    if (!signedIn) {
      throw redirect({ to: "/sign-in", search: { redirect: location.href } });
    }
    // Awaited, so the page's own calls are traced too.
    await recordDiagnosticsIfExported(context);
    return { signedIn };
  },
  component: Member,
});

function Member() {
  const { signedIn } = Route.useRouteContext();
  return <MemberLayout user={signedIn.user} />;
}
