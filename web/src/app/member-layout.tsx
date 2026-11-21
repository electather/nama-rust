import { Outlet, useMatches } from "@tanstack/react-router";
import type { ComponentProps } from "react";

import { useEvents } from "@/app/data/use-events";
import { SettingsLoader } from "@/app/settings-loader";
import { MemberChrome } from "@/app/top-bar/member-chrome";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { ConfirmItsYou } from "@/shared/components/confirm-its-you/confirm-its-you";

/**
 * Pages for the signed-in `user`, under the app chrome unless they fill the screen, with Settings
 * over them while the address asks for it.
 */
export function MemberLayout({
  chrome,
  user,
}: {
  chrome: ComponentProps<typeof MemberChrome>;
  user: User | undefined;
}) {
  // A full-screen page that didn't load, such as an unknown Movie's player, keeps the chrome, so
  // its not-found or error page has a way out.
  const fullScreen = useMatches({
    select: (matches) =>
      matches.some((m) => m.staticData.fullScreen === true && m.status === "success"),
  });
  useEvents();
  return (
    <ConfirmItsYou>
      {!fullScreen && <MemberChrome {...chrome} />}
      <Outlet />
      <SettingsLoader user={user} />
    </ConfirmItsYou>
  );
}
