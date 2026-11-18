import { FilterControls } from "@/features/library/components/filter-controls";
import { SortControls } from "@/features/library/components/sort-controls";
import type { BarProps } from "@/features/library/lib/listing-bar";

/**
 * The sort and filter bar over a Library's grid: sort, its direction, any of `genres`, a year
 * range and watched state. `change` gets the listing with what the User changed.
 */
export function TitleListControls({ genres, ...bar }: BarProps & { genres: string[] }) {
  return (
    <div className="-ms-4 mt-6 flex flex-wrap items-center gap-x-1 gap-y-2">
      <SortControls {...bar} />
      <FilterControls genres={genres} {...bar} />
    </div>
  );
}
