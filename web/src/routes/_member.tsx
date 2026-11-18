import { createFileRoute, redirect } from "@tanstack/react-router";

import { MemberLayout } from "@/app/member-layout";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { adminPagesFor, isSetupRequired, whoIsSignedIn } from "@/lib/access";
import { recordDiagnosticsIfExported } from "@/lib/diagnostics";
import { may } from "@/shared/lib/permissions";

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
    const chrome = {
      hasAdminPages: adminPagesFor(signedIn.user).length > 0,
      managesRequests: may(signedIn.user, Permission.MANAGE_REQUESTS),
    };
    return { signedIn, chrome };
  },
  component: Member,
});

function Member() {
  const { chrome } = Route.useRouteContext();
  return <MemberLayout chrome={chrome} />;
}
