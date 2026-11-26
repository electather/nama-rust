import { Outlet, useMatches, useSearch } from "@tanstack/react-router";

import { SettingsLoader } from "@/app/settings-loader";
import { SignedOutLayout } from "@/shared/components/sign-in-layout/signed-out-layout";

/**
 * The signed-in Account's pages at the Hub, with the Account's Settings over them while the address
 * asks for it. A Server's pages, and those that say they fill the Hub, fill the screen, as they do
 * at the Server.
 */
export function HubMemberLayout() {
  const onAServer = useSearch({ strict: false, select: (search) => search.server !== undefined });
  const fills = useMatches({
    select: (matches) => matches.some((match) => match.staticData.fillsTheHub === true),
  });
  const settings = <SettingsLoader user={undefined} atHub />;
  // ponytail: they sit in the signed-out pages' frame, and a Server's pages under no app chrome,
  // until the Hub's Home gives them some (#2204).
  return onAServer || fills ? (
    <>
      <Outlet />
      {settings}
    </>
  ) : (
    <SignedOutLayout>
      <Outlet />
      {settings}
    </SignedOutLayout>
  );
}
