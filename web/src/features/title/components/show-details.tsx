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
 * many of them are here, Specials aside, of the `aired`, in what picture and how much room they
 * take.
 */
export function ShowDetails({
  libraryId,
  show,
  episodes,
  aired,
}: {
  libraryId: bigint;
  show: Show;
  episodes: Episode[];
  aired: number;
}) {
  const versions = episodes.flatMap((episode) => episode.versions);
  // Specials aside, as the Server counts the aired ones and the Seasons.
  const here = episodes.filter(({ seasonNumber }) => seasonNumber > 0).length;
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
          [["Episodes", episodesHere(here, aired, show.seasonCount)]],
        ),
      ]}
    />
  );
}
