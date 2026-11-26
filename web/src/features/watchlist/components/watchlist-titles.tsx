import { WatchlistCard } from "@/features/watchlist/components/watchlist-card";
import type { WatchlistEntry } from "@/features/watchlist/lib/merged-watchlist";
import { posterGrid } from "@/shared/lib/poster-classes";

/**
 * The Watchlist's titles as a Library's grid of posters, or, once every Server has answered (`loading`
 * false), that there are none.
 */
export function WatchlistTitles({
  entries,
  loading,
}: {
  entries: WatchlistEntry[];
  loading: boolean;
}) {
  if (entries.length === 0) {
    return loading ? null : (
      <div className="mt-8">
        <p className="font-medium">Nothing on your Watchlist yet</p>
        <p className="mt-1 max-w-prose text-muted-foreground">
          Add a Movie or Show with + on its page to keep it here for later.
        </p>
      </div>
    );
  }
  return (
    <ul aria-label="Your Watchlist" className={posterGrid}>
      {entries.map((entry) => (
        <WatchlistCard key={entry.key} entry={entry} />
      ))}
    </ul>
  );
}
