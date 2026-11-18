import type { MessageInitShape } from "@bufbuild/protobuf";

import { useAutoplayOn } from "@/features/player/data/use-autoplay-on";
import { useEpisodeAfter } from "@/features/player/data/use-episode-after";
import type { GetNextEpisodeRequestSchema } from "@/gen/nama/v1/browse_pb";

/** The Episode after the one `episode` names, if the User has autoplay on. */
export function useAutoplayedEpisode(
  episode: MessageInitShape<typeof GetNextEpisodeRequestSchema>,
) {
  // Neither suspends: the Episode plays while the Server finds the next one.
  const next = useEpisodeAfter(episode);
  // Off until the Server says it's on, so a User who turned it off never sees a countdown.
  const on = useAutoplayOn();
  return on ? next : undefined;
}
