import { LibraryLink } from "@/features/title/components/library-link";
import { PeopleLinks } from "@/features/title/components/people-links";
import { TitleDetails } from "@/features/title/components/title-details";
import { filesDetails } from "@/features/title/lib/files-details";
import { listed } from "@/features/title/lib/listed";
import { crewWithJob } from "@/features/title/lib/people";
import type { Movie } from "@/gen/nama/v1/browse_pb";
import { yearOf } from "@/shared/lib/title-facts";

/** A Movie's Details: when it came out and its rating, its languages, and its files here. */
export function MovieDetails({ libraryId, movie }: { libraryId: bigint; movie: Movie }) {
  const directors = crewWithJob(movie.crew, "Director");
  return (
    <TitleDetails
      groups={[
        {
          heading: "Information",
          facts: [
            ["Released", yearOf(movie.releaseDate)],
            ["Rated", movie.contentRating],
            ["Genres", listed(movie.genres)],
            ["Directed by", directors && <PeopleLinks key="crew" people={directors} />],
          ],
        },
        ...filesDetails(movie, <LibraryLink key="library" libraryId={libraryId} />),
      ]}
    />
  );
}
