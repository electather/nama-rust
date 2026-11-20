import { Check, Plus } from "lucide-react";

import { type OnWatchlist, useToggleWatchlist } from "@/features/home/data/use-toggle-watchlist";
import { Button } from "@/shared/ui/button";

/** Adds a title to your Watchlist, or, lit while it's on it, takes it off. */
export function WatchlistButton({ watchlist }: { watchlist: OnWatchlist }) {
  const { pending, toggle } = useToggleWatchlist();
  return (
    <Button
      variant="secondary"
      size="round"
      aria-label={watchlist.on ? "Remove from your Watchlist" : "Add to your Watchlist"}
      aria-pressed={watchlist.on}
      onClick={() => {
        // Pressed twice before the Server answers, it would ask the same thing twice.
        if (!pending) {
          toggle(watchlist);
        }
      }}
    >
      {watchlist.on ? <Check aria-hidden /> : <Plus aria-hidden />}
    </Button>
  );
}
