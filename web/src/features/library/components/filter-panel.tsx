import { ClearFilters } from "@/features/library/components/clear-filters";
import { FilterChoices } from "@/features/library/components/filter-choices";
import type { BarProps } from "@/features/library/lib/listing-bar";

/** What the Filter button opens: its choices, with Clear filters in the top end corner. */
export function FilterPanel(props: BarProps & { genres: string[] }) {
  return (
    // Clear filters sits outside the scroller, so it stays in reach however far that's scrolled.
    <div className="relative flex min-h-0 flex-col">
      <FilterChoices {...props} />
      {/* Over the panel, so coming and going moves nothing, centred on its first heading's line,
          where it clears the chips under it. */}
      <div className="absolute inset-e-1.5 top-1.5">
        <ClearFilters {...props} />
      </div>
    </div>
  );
}
