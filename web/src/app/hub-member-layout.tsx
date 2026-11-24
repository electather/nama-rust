import { Outlet } from "@tanstack/react-router";

import { SettingsLoader } from "@/app/settings-loader";
import { SignedOutLayout } from "@/shared/components/sign-in-layout/signed-out-layout";

/**
 * The signed-in Account's pages at the Hub, with the Account's Settings over them while the address
 * asks for it.
 */
export function HubMemberLayout() {
  // ponytail: they sit in the signed-out pages' frame, without app chrome, until the Hub's Home
  // gives them some (#2204).
  return (
    <SignedOutLayout>
      <Outlet />
      <SettingsLoader user={undefined} atHub />
    </SignedOutLayout>
  );
}
