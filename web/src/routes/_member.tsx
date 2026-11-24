import { createFileRoute, redirect } from "@tanstack/react-router";

import { MemberLayout } from "@/app/member-layout";
import { type Section, idParam, sectionParam, userParam } from "@/features/settings/lib/sections";
import { serverInfo, whoIsSignedIn } from "@/shared/hooks/access";
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
    // Asked together, so the page's own calls wait on one round trip, not two. Who is signed in is
    // awaited only after the setup check, so a fresh Server still sends you to /setup if it fails.
    const whoAmI = whoIsSignedIn(context);
    // Handled now, so a failure while the Server's info loads isn't unhandled; awaited below.
    void whoAmI.catch(() => null);
    const info = await serverInfo(context);
    if (info.setupRequired) {
      throw redirect({ to: "/setup" });
    }
    const signedIn = await whoAmI;
    if (!signedIn) {
      throw redirect({ to: "/sign-in", search: { redirect: location.href } });
    }
    // Awaited, so the page's own calls are traced too.
    await recordDiagnosticsIfExported(info.exportsDiagnostics);
    return { signedIn };
  },
  component: Member,
});

function Member() {
  const { signedIn } = Route.useRouteContext();
  return <MemberLayout chrome={{ userName: signedIn.user?.name ?? "" }} user={signedIn.user} />;
}
