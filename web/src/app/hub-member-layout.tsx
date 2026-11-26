import { Outlet } from "@tanstack/react-router";

import { AppChrome } from "@/app/app-chrome";
import { SettingsLoader } from "@/app/settings-loader";

/**
 * The pages of the Account with `email` signed in at the Hub, under the Server's app chrome, which
 * stays the Hub's on a Server's pages too, with the Account's Settings over them while the address
 * asks for it. It follows no Server's Events and asks no "Confirm it’s you": a Server's pages ask
 * their own (`ServerLayout`).
 */
export function HubMemberLayout({ email }: { email: string }) {
  return (
    <>
      <AppChrome userName={email} atHub />
      <Outlet />
      <SettingsLoader user={undefined} atHub />
    </>
  );
}
