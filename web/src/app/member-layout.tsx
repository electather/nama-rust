import { Outlet, useMatches } from "@tanstack/react-router";
import type { ComponentProps } from "react";

import { useEvents } from "@/app/data/use-events";
import { MemberChrome } from "@/app/top-bar/member-chrome";
import { ConfirmItsYou } from "@/shared/components/confirm-its-you/confirm-its-you";

/** Pages for a signed-in User, under the app chrome unless they fill the screen. */
export function MemberLayout({ chrome }: { chrome: ComponentProps<typeof MemberChrome> }) {
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
    </ConfirmItsYou>
  );
}
