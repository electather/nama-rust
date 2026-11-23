import { createFileRoute, redirect } from "@tanstack/react-router";

import { MemberLayout } from "@/app/member-layout";
import { type Section, idParam, sectionParam, userParam } from "@/features/settings/lib/sections";
import { isSetupRequired, serverInfo, whoIsSignedIn } from "@/shared/hooks/access";
import { recordDiagnosticsIfExported } from "@/shared/lib/diagnostics";
import { wordsParam } from "@/shared/lib/words-param";

// On any page: the words the action bar opens on (`src/app/top-bar/use-search-from-url.ts`), the
// section Settings is open on (`src/app/settings-loader.tsx`), the Plugin, User, Job or Library open
// in its Plugins, Users, Jobs or Libraries section, and the setting a search found in it.
interface MemberSearch {
  search?: string | undefined;
  settings?: Section | undefined;
  plugin?: string | undefined;
  user?: number | undefined;
  library?: string | undefined;
  job?: string | undefined;
  setting?: string | undefined;
}

/** Pages for signed-in Users. */
export const Route = createFileRoute("/_member")({
  validateSearch: (search): MemberSearch => ({
    search: wordsParam(search["search"]),
    settings: sectionParam(search["settings"]),
    plugin: idParam(search["plugin"]),
    user: userParam(search["user"]),
    library: idParam(search["library"]),
    job: idParam(search["job"]),
    setting: idParam(search["setting"]),
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
    return { signedIn };
  },
  component: Member,
});

function Member() {
  const { signedIn } = Route.useRouteContext();
  return <MemberLayout chrome={{ userName: signedIn.user?.name ?? "" }} user={signedIn.user} />;
}
