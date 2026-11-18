import { Outlet, useMatches } from "@tanstack/react-router";

import type { chromeFor } from "@/app/chrome";
import { useEvents } from "@/app/data/use-events";
import { TopBar } from "@/app/top-bar/top-bar";
import { TopBarEnd } from "@/app/top-bar/top-bar-end";
import { TopBarStart } from "@/app/top-bar/top-bar-start";
import { useActionBarOpening } from "@/app/use-action-bar-opening";
import { ActionBar } from "@/features/action-bar/components/action-bar";
import { ConfirmItsYou } from "@/shared/components/confirm-its-you/confirm-its-you";

/**
 * Pages for a signed-in User, under the top bar unless they fill the screen, with the action bar.
 * `search`, from the URL, opens the action bar with it typed.
 */
export function MemberLayout({
  chrome,
  search,
}: {
  chrome: ReturnType<typeof chromeFor>;
  search: string | undefined;
}) {
  // A full-screen page that didn't load, such as an unknown Movie's player, keeps the top bar, so
  // its not-found or error page has a way out.
  const fullScreen = useMatches({
    select: (matches) =>
      matches.some((m) => m.staticData.fullScreen === true && m.status === "success"),
  });
  const [opening, setOpening] = useActionBarOpening(search);
  useEvents();
  return (
    <ConfirmItsYou>
      {!fullScreen && (
        <TopBar>
          <TopBarStart
            hasAdminPages={chrome.hasAdminPages}
            managesRequests={chrome.managesRequests}
          />
          <TopBarEnd
            userName={chrome.userName}
            onSearch={() => {
              setOpening({ frame: "root", query: "" });
            }}
          />
        </TopBar>
      )}
      <Outlet />
      <ActionBar
        opening={opening}
        hasAdminPages={chrome.hasAdminPages}
        shortcuts={!fullScreen}
        onOpen={setOpening}
        onClose={() => {
          setOpening(undefined);
        }}
      />
    </ConfirmItsYou>
  );
}
