import { Link } from "@tanstack/react-router";
import { Info } from "lucide-react";
import { buttonVariants } from "nama-ui/components/button";

import type { Panel } from "@/features/home/lib/featured-panels";
import { PlayButton } from "@/shared/components/watch-state/play-button";
import { WatchlistButton } from "@/shared/components/watchlist-button";

/** An open panel's Play or Resume, or More info with nothing to play, and its Watchlist button. */
export function FeaturedActions({ panel }: { panel: Panel }) {
  const { action } = panel;
  return (
    <div className="pointer-events-auto flex flex-wrap gap-2 pt-2">
      {action.kind === "play" ? (
        <PlayButton link={action.link} state={action.state} />
      ) : (
        <Link {...action.link} className={buttonVariants({ size: "tall" })}>
          <Info aria-hidden />
          More info
        </Link>
      )}
      <WatchlistButton watchlist={panel.watchlist} />
    </div>
  );
}
