import { LibraryLink } from "@/features/title/components/library-link";
import { TitleDetails } from "@/features/title/components/title-details";
import { crewWithJob } from "@/features/title/lib/crew-with-job";
import { episodesHere } from "@/features/title/lib/episodes-here";
import { filesDetails } from "@/features/title/lib/files-details";
import { listed } from "@/features/title/lib/listed";
import type { Episode, Show } from "@/gen/nama/v1/browse_pb";
import { yearOf } from "@/shared/lib/title-facts";

/**
 * A Show's Details: when it first aired and its rating, the languages of its Episodes, and how
 * many of them are here, in what picture and how much room they take.
 */
export function ShowDetails({
  libraryId,
  show,
  episodes,
}: {
  libraryId: bigint;
  show: Show;
  episodes: Episode[];
}) {
  const versions = episodes.flatMap((episode) => episode.versions);
  return (
    <TitleDetails
      groups={[
        {
          heading: "Information",
          facts: [
            ["Released", yearOf(show.firstAirDate)],
            ["Rated", show.contentRating],
            ["Genres", listed(show.genres)],
            ["Created by", crewWithJob(show.crew, "Creator")],
          ],
        },
        ...filesDetails(
          { versions, originalLanguage: show.originalLanguage },
          <LibraryLink key="library" libraryId={libraryId} />,
          [["Episodes", episodesHere(episodes.length, show.seasonCount)]],
        ),
      ]}
    />
  );
}
