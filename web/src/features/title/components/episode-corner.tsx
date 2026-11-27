import { AskCorner } from "@/features/title/components/ask-corner";
import { RingCorner } from "@/features/title/components/ring-corner";
import { StateCorner } from "@/features/title/components/state-corner";
import { type Standing, seasonStanding } from "@/shared/lib/request-standing";

/**
 * Where a missing Episode's Season `season` is on its way to the Server, by the Episode's
 * `standing`, in its card's top corner: + to Request it while that opens a Request, a ring filled
 * as far as its downloads have got, its Request's state's icon, such as a clock while Pending, or
 * nothing while its Season is in a Library.
 */
export function EpisodeCorner({ season, standing }: { season: number; standing: Standing }) {
  const ofSeason = seasonStanding(standing);
  if (ofSeason.can === "open") {
    return <AskCorner season={season} />;
  }
  if (ofSeason.kind === "downloading") {
    return <RingCorner downloads={ofSeason.request.downloads} />;
  }
  return "request" in ofSeason && <StateCorner state={ofSeason.request.state} />;
}
