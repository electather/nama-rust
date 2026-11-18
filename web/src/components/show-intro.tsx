import { useQuery } from "@connectrpc/connect-query";

import { Overview, TitleFacts } from "@/components/title-page";
import { BrowseService, type ContinueWatchingEntry, type Show } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { PlayOrResume } from "@/shared/components/watch-state/play-or-resume";
import { episodeName, metadataLine, timeLeft, yearOf } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/**
 * The top of a Show's page under its title: its genres, first air year and Content Rating, Resume
 * for its Episode in your Continue Watching if there is one, and its own overview.
 */
export function ShowIntro({
  libraryId,
  showId,
  show,
}: {
  libraryId: bigint;
  showId: ExternalId;
  show: Show;
}) {
  return (
    <>
      <TitleFacts
        facts={["Show", ...show.genres, yearOf(show.firstAirDate)]}
        boxed={[show.contentRating]}
      />
      <Resume libraryId={libraryId} showId={showId} />
      {show.overview !== undefined && <Overview text={show.overview} />}
    </>
  );
}

/** Resume or start the Show's Episode in your Continue Watching. Nothing without one. */
function Resume({ libraryId, showId }: { libraryId: bigint; showId: ExternalId }) {
  // ponytail: reads the first page, the 50 most recent; a Show started before those has no
  // Resume. Ask the Server for the Show's own resume Episode if that bites.
  const { data } = useQuery(BrowseService.method.listContinueWatching, {});
  const episode = episodeToResume(data?.entries ?? [], libraryId, showId);
  if (!episode) {
    return null;
  }
  const { seasonNumber, episodeNumber, watchState } = episode;
  const link = {
    to: "/libraries/$libraryId/shows/$showId/seasons/$season/episodes/$episode/play",
    params: {
      libraryId: String(libraryId),
      showId: titleParam(showId),
      season: String(seasonNumber),
      episode: String(episodeNumber),
    },
  } as const;
  return (
    <div className="mt-6 flex flex-wrap items-center gap-x-4 gap-y-3">
      <PlayOrResume
        link={link}
        state={watchState}
        detail={metadataLine(
          episodeName(seasonNumber, episodeNumber, "short"),
          timeLeft(watchState),
        )}
      />
    </div>
  );
}

/** The first of `entries`, most recent first, that's an Episode of `showId` in `libraryId`. */
function episodeToResume(entries: ContinueWatchingEntry[], libraryId: bigint, showId: ExternalId) {
  const inLibrary = entries.flatMap(
    ({ libraryId: entryLibrary, movieOrEpisode: { case: kind, value } }) =>
      entryLibrary === libraryId && kind === "episode" ? [value] : [],
  );
  return inLibrary.find(({ show }) => titleParam(show?.id) === titleParam(showId))?.episode;
}
