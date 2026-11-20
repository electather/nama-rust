import type { ContinueWatchingEntry } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { titleParam } from "@/shared/lib/title-param";

/** The first of `entries`, most recent first, that's an Episode of `showId` in `libraryId`. */
export function episodeToResume(
  entries: ContinueWatchingEntry[],
  libraryId: bigint,
  showId: ExternalId,
) {
  const inLibrary = entries.flatMap(
    ({ libraryId: entryLibrary, movieOrEpisode: { case: kind, value } }) =>
      entryLibrary === libraryId && kind === "episode" ? [value] : [],
  );
  return inLibrary.find(({ show }) => titleParam(show?.id) === titleParam(showId))?.episode;
}
