import { createFileRoute, redirect } from "@tanstack/react-router";

import { chromeFor } from "@/app/chrome";
import { MemberLayout } from "@/app/member-layout";
import { isSetupRequired, serverInfo, whoIsSignedIn } from "@/shared/hooks/access";
import { recordDiagnosticsIfExported } from "@/shared/lib/diagnostics";
import { wordsParam } from "@/shared/lib/words-param";

/** Pages for signed-in Users. `?search=` opens the action bar with it typed. */
export const Route = createFileRoute("/_member")({
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
    return { signedIn, chrome: chromeFor(signedIn.user) };
  },
  component: Member,
});

function Member() {
  const { chrome } = Route.useRouteContext();
  const { search } = Route.useSearch();
  return <MemberLayout chrome={chrome} search={search} />;
}
