import { Check, Plus } from "lucide-react";

import { type OnWatchlist, useToggleWatchlist } from "@/features/home/data/use-toggle-watchlist";
import { Button } from "@/shared/ui/button";

/**
 * Adds a title to your Watchlist, or, with a check while it's on it, takes it off. Its name says
 * which, so it isn't also `aria-pressed`, which would light it and say it twice.
 */
export function WatchlistButton({ watchlist }: { watchlist: OnWatchlist }) {
  const { on, toggle } = useToggleWatchlist(watchlist);
  return (
    <Button
      variant="secondary"
      size="round"
      aria-label={on ? "Remove from your Watchlist" : "Add to your Watchlist"}
      onClick={toggle}
    >
      {on ? <Check aria-hidden /> : <Plus aria-hidden />}
    </Button>
  );
}
