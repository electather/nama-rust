import { AskCorner } from "@/features/title/components/ask-corner";
import { RingCorner } from "@/features/title/components/ring-corner";
import { StateCorner } from "@/features/title/components/state-corner";
import { cornerShown } from "@/features/title/lib/episode-corner-shown";
import type { Request } from "@/gen/nama/v1/request_pb";

/**
 * Where a missing Episode's Season is on its way to the Server, in its card's top corner: + to
 * Request `season` while it has no `request`, or one Declined, a ring filled as far as its
 * downloads have got, or else its state's icon, such as a clock while Pending.
 */
export function EpisodeCorner({
  season,
  request,
}: {
  season: number;
  request: Request | undefined;
}) {
  const shown = cornerShown(request);
  if (shown.kind === "ask") {
    return <AskCorner season={season} />;
  }
  return shown.kind === "ring" ? (
    <RingCorner downloads={shown.downloads} />
  ) : (
    <StateCorner state={shown.state} />
  );
}
