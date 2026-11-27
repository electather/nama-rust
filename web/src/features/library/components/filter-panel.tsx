import { Button } from "nama-ui/components/button";

import { GenreChips } from "@/features/library/components/genre-chips";
import { WatchStateChoice } from "@/features/library/components/watch-state-choice";
import type { BarProps } from "@/features/library/lib/listing-bar";
import { isFiltered, unfiltered } from "@/features/library/lib/title-listing";

const heading = "text-sm font-medium";

/** What the Filter button opens: genres, then Watch State, and Clear filters. */
export function FilterPanel({ genres, ...bar }: BarProps & { genres: string[] }) {
  const clear = isFiltered(bar.listing) && (
    <div className="-mt-2 flex justify-end">
      <Button
        variant="ghost"
        onClick={() => {
          bar.change(unfiltered(bar.listing));
        }}
      >
        Clear filters
      </Button>
    </div>
  );
  return (
    // A short window scrolls the panel. While the popover eases to a new size it clips instead,
    // or a scrollbar coming and going narrows the chips, rewraps them and resizes it again.
    <div className="flex min-h-0 scrollbar-quiet flex-col gap-4 overflow-y-auto p-3 in-data-resizing:overflow-hidden">
      {genres.length > 0 && <GenreChips genres={genres} {...bar} />}
      <div className="flex flex-col gap-2">
        <p className={heading}>Watch State</p>
        <WatchStateChoice {...bar} />
      </div>
      {clear}
    </div>
  );
}
