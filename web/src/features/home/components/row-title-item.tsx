import type { RowTitle } from "@/features/home/lib/row-titles";
import { MovieInLibraryItem } from "@/shared/components/titles-in-library/movie-in-library-item";
import { ShowInLibraryItem } from "@/shared/components/titles-in-library/show-in-library-item";

/** A Movie or Show of a row, as the poster its Library's titles show. */
export function RowTitleItem({ title }: { title: RowTitle }) {
  if (title.case === "movie") {
    return <MovieInLibraryItem movie={title.value} />;
  }
  return <ShowInLibraryItem show={title.value} />;
}
