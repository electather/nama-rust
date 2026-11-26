// fallow-ignore-next-line policy-violation:feature-layout/data-hooks -- no query is built here: a title's poster reads its Server's image widths through its transport, as `_member/_server` hands a Server's pages theirs
import { TransportProvider } from "@connectrpc/connect-query";

import { RemoveFromWatchlist } from "@/features/watchlist/components/remove-from-watchlist";
import type { WatchlistEntry } from "@/features/watchlist/lib/merged-watchlist";
import { RowPoster } from "@/shared/components/row-poster/row-poster";
import { rowTile } from "@/shared/components/row-poster/row-tiles";

/**
 * A title on your Watchlist: its poster, as Home's rows draw it, opening its page on the Server
 * it shows from, and a ✕ that takes it off every Watchlist it's on.
 */
export function WatchlistCard({ entry }: { entry: WatchlistEntry }) {
  const tile = rowTile(entry.title);
  const link = entry.server ? { ...tile.link, search: { server: entry.server.id } } : tile.link;
  return (
    <li className="relative">
      <TransportProvider transport={entry.transport}>
        <RowPoster tile={{ ...tile, link }} />
      </TransportProvider>
      <RemoveFromWatchlist title={tile.title} listed={entry.listed} />
    </li>
  );
}
