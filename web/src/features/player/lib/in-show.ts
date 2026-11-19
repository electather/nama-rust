import type { LinkOptions } from "@tanstack/react-router";

import type { PlayedSeason } from "@/features/player/lib/played-season";
import type { Episode } from "@/gen/nama/v1/browse_pb";

/** Where an Episode plays in its Show: what the Episodes panel lists and Next Episode opens. */
export interface InShow {
  season: PlayedSeason;
  episodeNumber: number;
  /** The Episode after this one, once the Server has found it, whether autoplay is on or not. */
  after: Episode | undefined;
  link: (episode: Pick<Episode, "seasonNumber" | "episodeNumber">) => LinkOptions;
}

/** The play route of the Episode after this one, once the Server has found it. */
export function nextLink(inShow: InShow | undefined) {
  return inShow?.after && inShow.link(inShow.after);
}
