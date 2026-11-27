import { SortAndFilter } from "@/features/library/components/sort-and-filter";
import { TitleCounts } from "@/features/library/components/title-counts";
import { isFiltered } from "@/features/library/lib/title-listing";
import type { TitlesProps } from "@/features/library/lib/titles-props";

/** The sort and Filter over a Library's grid, unless the Library has no titles to sort. */
export function TitleControls({ kind, ...titles }: TitlesProps & { kind: "movies" | "shows" }) {
  return (
    <TitleCounts libraryId={titles.libraryId} listing={titles.listing} kind={kind}>
      {({ shown }) =>
        shown === 0 && !isFiltered(titles.listing) ? null : <SortAndFilter {...titles} />
      }
    </TitleCounts>
  );
}
