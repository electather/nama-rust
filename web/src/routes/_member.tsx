import { create } from "@bufbuild/protobuf";
import { createFileRoute, redirect } from "@tanstack/react-router";

import { HubMemberLayout } from "@/app/hub-member-layout";
import { MemberLayout } from "@/app/member-layout";
import { type Section, idParam, sectionParam, userParam } from "@/features/settings/lib/sections";
import { WhoAmIResponseSchema } from "@/gen/nama/v1/sign_in_pb";
import {
  accountOrSignIn,
  actingUserOrRedirect,
  isAtHub,
  serverInfo,
  whoIsSignedIn,
} from "@/shared/hooks/access";
import { recordDiagnosticsIfExported } from "@/shared/lib/diagnostics";
import { wordsParam } from "@/shared/lib/words-param";

// On any page: the words the action bar opens on (`src/app/top-bar/use-search-from-url.ts`), the
// section Settings is open on (`src/app/settings-loader.tsx`), the Plugin, User, Job or Library open
// in its Plugins, Users, Jobs or Libraries section, the setting a search found in it, and at the Hub,
// the Linked Server whose Settings show.
interface MemberSearch {
  search?: string | undefined;
  settings?: Section | undefined;
  manage?: string | undefined;
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
    manage: idParam(search["manage"]),
    plugin: idParam(search["plugin"]),
    user: userParam(search["user"]),
    library: idParam(search["library"]),
    job: idParam(search["job"]),
    setting: idParam(search["setting"]),
  }),
  beforeLoad: async ({ context, location }) => {
    // Asked together, so the page's own calls wait on one round trip, not two; at the Hub the
    // Server's two fail unread. Who is signed in is awaited only after the setup check, so a fresh
    // Server still sends you to /setup if it fails.
    const whoAmI = whoIsSignedIn(context);
    const serverAnswer = serverInfo(context);
    // Handled now, so a failure while another loads isn't unhandled; awaited below.
    void whoAmI.catch(() => null);
    void serverAnswer.catch(() => null);
    if (await isAtHub(context)) {
      const account = await accountOrSignIn(context, location.href);
      // No Server User is signed in at the Hub, so every Permission check says no.
      return { signedIn: create(WhoAmIResponseSchema), account };
    }
    const info = await serverAnswer;
    if (info.setupRequired) {
      throw redirect({ to: "/setup" });
    }
    const signedIn = actingUserOrRedirect(await whoAmI, location.href);
    // Awaited, so the page's own calls are traced too.
    await recordDiagnosticsIfExported(info.exportsDiagnostics);
    return { signedIn, account: null };
  },
  component: Member,
});

function Member() {
  const { signedIn, account } = Route.useRouteContext();
  if (account) {
    return <HubMemberLayout />;
  }
  return <MemberLayout chrome={{ userName: signedIn.user?.name ?? "" }} user={signedIn.user} />;
}
