import type { ServerChoice } from "@/features/title/lib/server-choices";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { PlayButton } from "@/shared/components/watch-state/play-button";
import { episodeName } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/**
 * Play or Resume `episode` of the Show `showId` on the Server holding `choice`, named for which
 * Episode it is.
 */
export function ShowPlay({
  showId,
  episode,
  choice,
}: {
  showId: ExternalId;
  episode: Episode;
  choice: ServerChoice;
}) {
  const { seasonNumber, episodeNumber } = episode;
  return (
    <PlayButton
      link={{
        to: "/libraries/$libraryId/shows/$showId/seasons/$season/episodes/$episode/play",
        params: {
          libraryId: String(choice.libraryId),
          showId: titleParam(showId),
          season: String(seasonNumber),
          episode: String(episodeNumber),
        },
        search: { server: choice.server?.id },
      }}
      state={choice.watchState}
      what={episodeName(seasonNumber, episodeNumber, "full")}
    />
  );
}
