import { Suspense } from "react";

import { HubWatchlist } from "@/features/watchlist/components/hub-watchlist";
import { ServerWatchlist } from "@/features/watchlist/components/server-watchlist";
import { Page } from "@/shared/components/page";

/** Your Watchlist: this Server's, or at the Hub, every one of the Account's Servers' merged. */
export function WatchlistPage({ atHub }: { atHub: boolean }) {
  return (
    <Page heading="Watchlist">
      <Suspense fallback={null}>{atHub ? <HubWatchlist /> : <ServerWatchlist />}</Suspense>
    </Page>
  );
}
