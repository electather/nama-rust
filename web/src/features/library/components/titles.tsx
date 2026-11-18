import { EmptyLibrary } from "@/features/library/components/empty-library";
import { TitleCount } from "@/features/library/components/title-count";
import { TitleGrid } from "@/features/library/components/title-grid";
import { TitleListControls } from "@/features/library/components/title-list-controls";
import { useGenres } from "@/features/library/data/use-genres";
import { isFiltered } from "@/features/library/lib/title-listing";
import type { GridProps, TitlesProps } from "@/features/library/lib/titles-props";

/**
 * A Library's titles so far: how many there are, the bar to sort and filter them, and their
 * posters, loading the next page as the grid's end scrolls into view. An empty Library says so
 * instead.
 */
export function Titles({ libraryId, listing, change, ...grid }: TitlesProps & GridProps) {
  const genres = useGenres(libraryId);
  if (grid.posters.length === 0 && !isFiltered(listing)) {
    return <EmptyLibrary titles={grid.titles.many} />;
  }
  return (
    <>
      <TitleCount count={grid.paged.totalSize} titles={grid.titles} />
      <TitleListControls listing={listing} genres={genres} change={change} />
      <TitleGrid {...grid} />
    </>
  );
}
