import { create } from "@bufbuild/protobuf";

import type { ShowSummary } from "@/gen/nama/v1/browse_pb";
import { WatchStateSchema } from "@/gen/nama/v1/watch_state_pb";

/**
 * Your Episodes of `show` played, as a Watch State the poster shows: its bar is the share played
 * and its mark says you've played them all.
 */
export function episodesWatched({ episodeCount, playedEpisodeCount }: ShowSummary) {
  return create(WatchStateSchema, {
    positionMs: BigInt(playedEpisodeCount),
    durationMs: BigInt(episodeCount),
    played: episodeCount > 0 && playedEpisodeCount === episodeCount,
  });
}
