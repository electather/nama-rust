import { EmptyLibrary } from "@/features/library/components/empty-library";
import { RailedGrid } from "@/features/library/components/railed-grid";
import { UnmatchedTile } from "@/features/library/components/unmatched-tile";
import { useUnmatched } from "@/features/library/data/use-unmatched";
import type { IndexKey } from "@/features/library/lib/rail";
import { isFiltered, unfiltered } from "@/features/library/lib/title-listing";
import type { GridProps, TitlesProps } from "@/features/library/lib/titles-props";
import { keptTitles, withUnmatched } from "@/features/library/lib/with-unmatched";

/**
 * A Library's titles so far, their posters beside the quick-scroll rail of `index`, then its
 * Unmatched Items as tiles, loading the next page as the grid's end scrolls into view. With Not
 * matched on, only those. An empty Library says so instead.
 */
export function Titles({
  libraryId,
  listing,
  change,
  index,
  titles,
  paged,
  posters,
}: TitlesProps & GridProps & { index: IndexKey[] }) {
  const joined = withUnmatched(paged, useUnmatched(libraryId), listing);
  const tiles = joined.items.map((item) => ({
    key: String(item.itemId),
    poster: <UnmatchedTile item={item} libraryId={libraryId} />,
  }));
  const shown = [...keptTitles(joined, posters), ...tiles];
  if (shown.length === 0 && !isFiltered(listing)) {
    return <EmptyLibrary titles={titles.many} />;
  }
  return (
    <RailedGrid
      // A new listing starts with no jump on its way.
      key={JSON.stringify(listing)}
      // The rail's letters name titles, which Not matched lists none of.
      index={keptTitles(joined, index)}
      byYear={listing.sort === "released"}
      clear={() => {
        change(unfiltered(listing));
      }}
      titles={titles}
      paged={joined.paged}
      posters={shown}
    />
  );
}
