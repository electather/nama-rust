import { Outlet } from "@tanstack/react-router";

import { ConfirmItsYou } from "@/shared/components/confirm-its-you/confirm-its-you";

/** One Server's pages, with their own "Confirm it’s you" at the Hub. */
export function ServerLayout({ atHub }: { atHub: boolean }) {
  // A Server's own pages have theirs in `MemberLayout`; the Hub's have none.
  return atHub ? (
    <ConfirmItsYou>
      <Outlet />
    </ConfirmItsYou>
  ) : (
    <Outlet />
  );
}
