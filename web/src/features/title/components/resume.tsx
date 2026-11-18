import { useContinueWatchingEntries } from "@/features/title/data/use-continue-watching-entries";
import type { ContinueWatchingEntry } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { PlayOrResume } from "@/shared/components/watch-state/play-or-resume";
import { episodeName, metadataLine, timeLeft } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/** Resume or start the Show's Episode in your Continue Watching. Nothing without one. */
export function Resume({ libraryId, showId }: { libraryId: bigint; showId: ExternalId }) {
  // ponytail: reads the first page, the 50 most recent; a Show started before those has no
  // Resume. Ask the Server for the Show's own resume Episode if that bites.
  const entries = useContinueWatchingEntries();
  const episode = episodeToResume(entries, libraryId, showId);
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
