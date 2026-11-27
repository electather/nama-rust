import { EmptyLibrary } from "@/features/library/components/empty-library";
import { RailedGrid } from "@/features/library/components/railed-grid";
import { TitleCount } from "@/features/library/components/title-count";
import { TitleListControls } from "@/features/library/components/title-list-controls";
import { useGenres } from "@/features/library/data/use-genres";
import type { IndexKey } from "@/features/library/lib/rail";
import { isFiltered, unfiltered } from "@/features/library/lib/title-listing";
import type { GridProps, TitlesProps } from "@/features/library/lib/titles-props";

/**
 * A Library's titles so far: how many there are, the bar to sort and filter them, and their
 * posters beside the quick-scroll rail of `index`, loading the next page as the grid's end
 * scrolls into view. An empty Library says so instead.
 */
export function Titles({
  libraryId,
  listing,
  change,
  index,
  ...grid
}: TitlesProps & GridProps & { index: IndexKey[] }) {
  const genres = useGenres(libraryId);
  if (grid.posters.length === 0 && !isFiltered(listing)) {
    return <EmptyLibrary titles={grid.titles.many} />;
  }
  return (
    <>
      <TitleCount count={grid.paged.totalSize} titles={grid.titles} />
      <TitleListControls listing={listing} genres={genres} change={change} />
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
    </>
  );
}
