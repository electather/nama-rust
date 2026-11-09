import { Outlet, createFileRoute, redirect, useMatches } from "@tanstack/react-router";

import { AppChrome } from "@/components/app-chrome";
import { ConfirmItsYou } from "@/components/confirm-its-you";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { adminPagesFor, isSetupRequired, whoIsSignedIn } from "@/lib/access";
import { useEvents } from "@/lib/events";
import { may } from "@/lib/permissions";

/** Pages for signed-in Users, under the app chrome unless they fill the screen. */
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
  // A full-screen page that didn't load, such as an unknown Movie's player, keeps the chrome, so
  // its not-found or error page has a way out.
  const fullScreen = useMatches({
    select: (matches) =>
      matches.some((m) => m.staticData.fullScreen === true && m.status === "success"),
  });
  useEvents();
  return (
    <ConfirmItsYou>
      {!fullScreen && (
        <AppChrome
          hasAdminPages={adminPagesFor(signedIn.user).length > 0}
          managesRequests={may(signedIn.user, Permission.MANAGE_REQUESTS)}
        />
      )}
      <Outlet />
    </ConfirmItsYou>
  );
}
