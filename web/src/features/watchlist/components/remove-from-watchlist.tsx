import { X } from "lucide-react";
import { Button } from "nama-ui/components/button";

import { useRemoveFromWatchlist } from "@/features/watchlist/data/use-remove-from-watchlist";
import type { Listed } from "@/features/watchlist/lib/merged-watchlist";

/**
 * The ✕ in a Watchlist poster's top end corner that takes `title` off each Watchlist it's
 * `listed` on. It sits on the scrim, as a missing title's badge does in the other corner, so it
 * reads over any art.
 */
export function RemoveFromWatchlist({ title, listed }: { title: string; listed: Listed[] }) {
  const remove = useRemoveFromWatchlist();
  return (
    <span className="dark absolute inset-e-2 top-2 rounded-lg bg-scrim">
      <Button
        variant="overlay"
        size="icon-sm"
        aria-label={`Remove ${title} from your Watchlist`}
        disabled={remove.isPending}
        onClick={() => {
          remove.mutate(listed);
        }}
      >
        <X aria-hidden />
      </Button>
    </span>
  );
}
