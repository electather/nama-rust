import { useShowOfEpisode } from "@/features/player/data/use-show-of-episode";
import type { PlayedSeason } from "@/features/player/lib/played-season";
import { episodeName, metadataLine } from "@/shared/lib/title-facts";

/**
 * The title bar of Episode `episodeNumber`, titled `title`: its Show, once that arrives, over which
 * Episode it is, its title and the Show's Content Rating.
 */
export function useEpisodeHeading(input: PlayedSeason, episodeNumber: number, title = "") {
  const show = useShowOfEpisode(input.libraryId, input.showId);
  return {
    heading: show?.title,
    line: metadataLine(
      episodeName(input.seasonNumber, episodeNumber, "short"),
      title,
      show?.contentRating,
    ),
  };
}
