import { Outlet, useSearch } from "@tanstack/react-router";

import { SettingsLoader } from "@/app/settings-loader";
import { ActionBarHost } from "@/app/top-bar/action-bar-host";
import { SearchButton } from "@/app/top-bar/search-button";
import { SignedOutLayout } from "@/shared/components/sign-in-layout/signed-out-layout";

/**
 * The signed-in Account's pages at the Hub, with the Account's Settings over them while the address
 * asks for it, and the action bar searching all its Servers. A Server's pages fill the screen, as
 * they do at the Server, where only the action bar's keys open it.
 */
export function HubMemberLayout() {
  const onAServer = useSearch({ strict: false, select: (search) => search.server !== undefined });
  const settings = <SettingsLoader user={undefined} atHub />;
  // ponytail: they sit in the signed-out pages' frame, and a Server's pages under no app chrome,
  // until the Hub's Home gives them some (#2204).
  const pages = (onSearch: () => void) =>
    onAServer ? (
      <>
        <Outlet />
        {settings}
      </>
    ) : (
      <SignedOutLayout>
        <div className="flex justify-center">
          <SearchButton label="Search your Servers" onSearch={onSearch} />
        </div>
        <Outlet />
        {settings}
      </SignedOutLayout>
    );
  return <ActionBarHost>{pages}</ActionBarHost>;
}
