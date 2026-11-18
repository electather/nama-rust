import { Outlet, useMatches } from "@tanstack/react-router";

import { AppChrome } from "@/app/app-chrome/app-chrome";
import { Permission, type User } from "@/gen/nama/v1/sign_in_pb";
import { adminPagesFor } from "@/lib/access";
import { useEvents } from "@/lib/events";
import { ConfirmItsYou } from "@/shared/components/confirm-its-you/confirm-its-you";
import { may } from "@/shared/lib/permissions";

/** Pages for the signed-in `user`, under the app chrome unless they fill the screen. */
export function MemberLayout({ user }: { user: User | undefined }) {
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
          hasAdminPages={adminPagesFor(user).length > 0}
          managesRequests={may(user, Permission.MANAGE_REQUESTS)}
        />
      )}
      <Outlet />
    </ConfirmItsYou>
  );
}
