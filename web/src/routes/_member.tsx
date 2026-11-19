import { createFileRoute, redirect } from "@tanstack/react-router";

import { MemberLayout } from "@/app/member-layout";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { adminPagesFor, isSetupRequired, serverInfo, whoIsSignedIn } from "@/shared/hooks/access";
import { recordDiagnosticsIfExported } from "@/shared/lib/diagnostics";
import { may } from "@/shared/lib/permissions";
import { wordsParam } from "@/shared/lib/words-param";

/** Pages for signed-in Users. */
export const Route = createFileRoute("/_member")({
  // The words the action bar opens on (`src/app/top-bar/use-search-from-url.ts`), on any page.
  validateSearch: (search): { search?: string | undefined } => ({
    search: wordsParam(search["search"]),
  }),
  beforeLoad: async ({ context, location }) => {
    if (await isSetupRequired(context)) {
      throw redirect({ to: "/setup" });
    }
    const signedIn = await whoIsSignedIn(context);
    if (!signedIn) {
      throw redirect({ to: "/sign-in", search: { redirect: location.href } });
    }
    // Awaited, so the page's own calls are traced too.
    await recordDiagnosticsIfExported((await serverInfo(context)).exportsDiagnostics);
    const chrome = {
      hasAdminPages: adminPagesFor(signedIn.user).length > 0,
      managesRequests: may(signedIn.user, Permission.MANAGE_REQUESTS),
    };
    return { signedIn, chrome };
  },
  component: Member,
});

function Member() {
  const { chrome, signedIn } = Route.useRouteContext();
  return <MemberLayout chrome={{ ...chrome, userName: signedIn.user?.name ?? "" }} />;
}
