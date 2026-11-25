import type { Copy } from "@/features/title/lib/copies";
import type { Episode } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { PlayButton } from "@/shared/components/watch-state/play-button";
import { episodeName } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/**
 * Play or Resume `episode` of the Show `showId` on the Server holding `copy`, named for which
 * Episode it is.
 */
export function ShowPlay({
  showId,
  episode,
  copy,
}: {
  showId: ExternalId;
  episode: Episode;
  copy: Copy;
}) {
  const { seasonNumber, episodeNumber } = episode;
  return (
    <PlayButton
      link={{
        to: "/libraries/$libraryId/shows/$showId/seasons/$season/episodes/$episode/play",
        params: {
          libraryId: String(copy.libraryId),
          showId: titleParam(showId),
          season: String(seasonNumber),
          episode: String(episodeNumber),
        },
        search: { server: copy.server?.id },
      }}
      state={copy.watchState}
      what={episodeName(seasonNumber, episodeNumber, "full")}
    />
  );
}
