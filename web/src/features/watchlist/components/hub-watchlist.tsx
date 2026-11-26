import { WatchlistTitles } from "@/features/watchlist/components/watchlist-titles";
import { useHubWatchlist } from "@/features/watchlist/data/use-hub-watchlist";

/** Your Watchlist on every one of the Account's Servers the browser reaches, each title once. */
export function HubWatchlist() {
  const { entries, loading } = useHubWatchlist();
  return <WatchlistTitles entries={entries} loading={loading} />;
}
