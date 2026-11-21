import { createFileRoute, redirect } from "@tanstack/react-router";

import { MemberLayout } from "@/app/member-layout";
import { type Section, pluginParam, sectionParam } from "@/features/settings/lib/sections";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { adminPagesFor, isSetupRequired, serverInfo, whoIsSignedIn } from "@/shared/hooks/access";
import { recordDiagnosticsIfExported } from "@/shared/lib/diagnostics";
import { may } from "@/shared/lib/permissions";
import { wordsParam } from "@/shared/lib/words-param";

// On any page: the words the action bar opens on (`src/app/top-bar/use-search-from-url.ts`), the
// section Settings is open on (`src/app/settings-loader.tsx`), and the Plugin open in its Plugins
// section.
interface MemberSearch {
  search?: string | undefined;
  settings?: Section | undefined;
  plugin?: string | undefined;
}

/** Pages for signed-in Users. */
export const Route = createFileRoute("/_member")({
  validateSearch: (search): MemberSearch => ({
    search: wordsParam(search["search"]),
    settings: sectionParam(search["settings"]),
    plugin: pluginParam(search["plugin"]),
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
  return (
    <MemberLayout
      chrome={{ ...chrome, userName: signedIn.user?.name ?? "" }}
      user={signedIn.user}
    />
  );
}
