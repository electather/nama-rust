import { WatchlistTitles } from "@/features/watchlist/components/watchlist-titles";
import { useWatchlist } from "@/features/watchlist/data/use-watchlist";

/** Your Watchlist on this Server. */
export function ServerWatchlist() {
  return <WatchlistTitles entries={useWatchlist()} loading={false} />;
}
