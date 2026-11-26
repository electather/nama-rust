import { createFileRoute } from "@tanstack/react-router";

import { WatchlistPage } from "@/features/watchlist/components/watchlist-page";
import { watchlistQuery } from "@/features/watchlist/data/use-watchlist";

/** Your Watchlist: at the Hub, every one of the Account's Servers' merged (ADR 0105). */
export const Route = createFileRoute("/_member/watchlist")({
  loader: async ({ context: { queryClient, transport, account } }) => {
    // At the Hub, each Server's loads on its own, so none holds up the rest.
    if (!account) {
      await queryClient.query(watchlistQuery(transport));
    }
  },
  staticData: { fillsTheHub: true },
  component: Watchlist,
});

function Watchlist() {
  const { account } = Route.useRouteContext();
  return <WatchlistPage atHub={account !== null} />;
}
