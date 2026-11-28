import { RemoveFromWatchlist } from "@/features/watchlist/components/remove-from-watchlist";
import type { WatchlistEntry } from "@/features/watchlist/lib/merged-watchlist";
import { OnServer } from "@/shared/components/on-server";
import { Poster } from "@/shared/components/poster/poster";
import { rowTile } from "@/shared/components/poster/row-tiles";
import { linkOn } from "@/shared/lib/title-server";

/**
 * A title on your Watchlist: its poster, as Home's rows draw it, opening its page on the Server
 * it shows from, and a ✕ that takes it off every Watchlist it's on.
 */
export function WatchlistCard({ entry }: { entry: WatchlistEntry }) {
  const tile = rowTile(entry.title);
  return (
    <li className="relative">
      <OnServer on={entry}>
        <Poster {...tile} link={linkOn(tile.link, entry)} size="tile" />
      </OnServer>
      <RemoveFromWatchlist title={tile.title} listed={entry.listed} />
    </li>
  );
}
