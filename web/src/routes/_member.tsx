import { Outlet, createFileRoute, redirect } from "@tanstack/react-router";

import { AppChrome } from "@/components/app-chrome";
import { Role } from "@/gen/nama/v1/sign_in_pb";
import { isSetupRequired, whoIsSignedIn } from "@/lib/access";
import { useBrowseRefreshes } from "@/lib/events";

/** Pages for signed-in Users, under the app chrome. */
export const Route = createFileRoute("/_member")({
  beforeLoad: async ({ context, location }) => {
    if (await isSetupRequired(context)) throw redirect({ to: "/setup" });
    const signedIn = await whoIsSignedIn(context);
    if (!signedIn) throw redirect({ to: "/sign-in", search: { redirect: location.href } });
    return { signedIn };
  },
  component: MemberLayout,
});

function MemberLayout() {
  const { signedIn } = Route.useRouteContext();
  useBrowseRefreshes();
  return (
    <>
      <AppChrome isAdmin={signedIn.user?.role === Role.ADMIN} />
      <Outlet />
    </>
  );
}
