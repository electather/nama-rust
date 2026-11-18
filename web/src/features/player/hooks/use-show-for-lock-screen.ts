import { useShowOfEpisode } from "@/features/player/data/use-show-of-episode";
import type { PlayedSeason } from "@/features/player/lib/played-season";
import { episodeName, metadataLine } from "@/shared/lib/title-facts";

/**
 * What the lock screen shows beside the Episode's title: its Show and which Episode it is, by the
 * Show's Poster. Nothing until the Show arrives, which the Episode doesn't wait for.
 */
export function useShowForLockScreen(
  { libraryId, showId, seasonNumber }: PlayedSeason,
  episodeNumber: number,
) {
  const show = useShowOfEpisode(libraryId, showId);
  return (
    show && {
      artist: metadataLine(show.title, episodeName(seasonNumber, episodeNumber, "short")),
      posterUrl: show.posterUrl,
    }
  );
}
