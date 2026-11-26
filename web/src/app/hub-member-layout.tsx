import { Outlet, useMatch, useMatches, useSearch } from "@tanstack/react-router";

import { SettingsLoader } from "@/app/settings-loader";
import { ActionBarHost } from "@/app/top-bar/action-bar-host";
import { SearchButton } from "@/app/top-bar/search-button";
import { SignedOutLayout } from "@/shared/components/sign-in-layout/signed-out-layout";

/**
 * The signed-in Account's pages at the Hub, with the Account's Settings over them while the address
 * asks for it, and the action bar searching all its Servers. Home, a Server's pages, and those that
 * say they fill the Hub, such as the Watchlist, fill the screen, as they do at the Server. On a
 * Server's pages, only its keys open the action bar.
 */
export function HubMemberLayout() {
  const onAServer = useSearch({ strict: false, select: (search) => search.server !== undefined });
  const atHome = useMatch({ from: "/_member/", shouldThrow: false }) !== undefined;
  const fills = useMatches({
    select: (matches) => matches.some((match) => match.staticData.fillsTheHub === true),
  });
  const settings = <SettingsLoader user={undefined} atHub />;
  // ponytail: the rest sit in the signed-out pages' frame, and all of them under no app chrome
  // but the search button, until the Hub gives them some (#2204).
  const fillsTheScreen = onAServer || atHome || fills;
  const pages = (onSearch: () => void) => {
    const page = (
      <>
        {!onAServer && (
          <div className="flex justify-center">
            <SearchButton label="Search your Servers" onSearch={onSearch} />
          </div>
        )}
        <Outlet />
        {settings}
      </>
    );
    return fillsTheScreen ? page : <SignedOutLayout>{page}</SignedOutLayout>;
  };
  return <ActionBarHost>{pages}</ActionBarHost>;
}
