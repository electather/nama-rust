import { MissingMovieItem } from "@/features/home/components/missing-movie-item";
import type { RowTitle } from "@/features/home/lib/row-titles";
import { ShowCard } from "@/shared/components/title-cards/show-card";
import { MovieInLibraryItem } from "@/shared/components/titles-in-library/movie-in-library-item";
import { ShowInLibraryItem } from "@/shared/components/titles-in-library/show-in-library-item";

/**
 * A title of a row: on the Server, as the poster its Library's titles show; missing from it, as a
 * search for titles to Request shows it.
 */
export function RowTitleItem({ title }: { title: RowTitle }) {
  if (title.case === "movie") {
    return <MovieInLibraryItem movie={title.value} />;
  }
  if (title.case === "show") {
    return <ShowInLibraryItem show={title.value} />;
  }
  if (title.case === "missingShow") {
    return <ShowCard show={title.value} />;
  }
  return <MissingMovieItem movie={title.value} />;
}
