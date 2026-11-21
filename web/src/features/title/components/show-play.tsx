import type { Episode } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { PlayButton } from "@/shared/components/watch-state/play-button";
import { episodeName } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/** Play or Resume `episode` of the Show `showId`, named for which Episode it is. */
export function ShowPlay({
  libraryId,
  showId,
  episode,
}: {
  libraryId: bigint;
  showId: ExternalId;
  episode: Episode;
}) {
  const { seasonNumber, episodeNumber } = episode;
  return (
    <PlayButton
      link={{
        to: "/libraries/$libraryId/shows/$showId/seasons/$season/episodes/$episode/play",
        params: {
          libraryId: String(libraryId),
          showId: titleParam(showId),
          season: String(seasonNumber),
          episode: String(episodeNumber),
        },
      }}
      state={episode.watchState}
      what={episodeName(seasonNumber, episodeNumber, "full")}
    />
  );
}
