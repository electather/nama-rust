import { EmptyLibrary } from "@/features/library/components/empty-library";
import { RailedGrid } from "@/features/library/components/railed-grid";
import type { IndexKey } from "@/features/library/lib/rail";
import { isFiltered, unfiltered } from "@/features/library/lib/title-listing";
import type { GridProps, TitlesProps } from "@/features/library/lib/titles-props";

/**
 * A Library's titles so far, their posters beside the quick-scroll rail of `index`, loading the
 * next page as the grid's end scrolls into view. An empty Library says so instead.
 */
export function Titles({
  listing,
  change,
  index,
  ...grid
}: Omit<TitlesProps, "libraryId"> & GridProps & { index: IndexKey[] }) {
  if (grid.posters.length === 0 && !isFiltered(listing)) {
    return <EmptyLibrary titles={grid.titles.many} />;
  }
  return (
    <RailedGrid
      // A new listing starts with no jump on its way.
      key={JSON.stringify(listing)}
      index={index}
      byYear={listing.sort === "released"}
      clear={() => {
        change(unfiltered(listing));
      }}
      {...grid}
    />
  );
}
