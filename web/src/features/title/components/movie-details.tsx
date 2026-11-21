import { LibraryLink } from "@/features/title/components/library-link";
import { TitleDetails } from "@/features/title/components/title-details";
import { filesDetails } from "@/features/title/lib/files-details";
import { listed } from "@/features/title/lib/listed";
import type { Movie } from "@/gen/nama/v1/browse_pb";
import { yearOf } from "@/shared/lib/title-facts";

/** A Movie's Details: when it came out and its rating, its languages, and its files here. */
export function MovieDetails({ libraryId, movie }: { libraryId: bigint; movie: Movie }) {
  return (
    <TitleDetails
      groups={[
        {
          heading: "Information",
          facts: [
            ["Released", yearOf(movie.releaseDate)],
            ["Rated", movie.contentRating],
            ["Genres", listed(movie.genres)],
          ],
        },
        ...filesDetails(movie, <LibraryLink key="library" libraryId={libraryId} />),
      ]}
    />
  );
}
