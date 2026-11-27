import { ClearFilters } from "@/features/library/components/clear-filters";
import { FilterPopover } from "@/features/library/components/filter-popover";
import { SortMenu } from "@/features/library/components/sort-menu";
import { useGenres } from "@/features/library/data/use-genres";
import type { BarProps } from "@/features/library/lib/listing-bar";

/**
 * The sort menu, the Filter button over a Library's grid, filtering by the Library's genres, and
 * the ✕ that clears its filters. What's on shows in the button, so turning a filter on moves
 * nothing.
 */
export function SortAndFilter({ libraryId, ...bar }: BarProps & { libraryId: bigint }) {
  const genres = useGenres(libraryId);
  return (
    <div className="flex items-center gap-1">
      <SortMenu {...bar} />
      <FilterPopover genres={genres} {...bar} />
      <ClearFilters {...bar} />
    </div>
  );
}
