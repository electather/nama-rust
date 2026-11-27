import { Outlet } from "@tanstack/react-router";
import type { ComponentProps } from "react";

import { AppChrome } from "@/app/app-chrome";
import { useEvents } from "@/app/data/use-events";
import { SettingsLoader } from "@/app/settings-loader";
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
  chrome: ComponentProps<typeof AppChrome>;
  user: User | undefined;
}) {
  useEvents();
  return (
    <ConfirmItsYou>
      <AppChrome {...chrome} />
      <Outlet />
      <SettingsLoader user={user} />
    </ConfirmItsYou>
  );
}
