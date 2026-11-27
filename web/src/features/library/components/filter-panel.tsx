import { Button } from "nama-ui/components/button";

import { GenreChips } from "@/features/library/components/genre-chips";
import { WatchStateChoice } from "@/features/library/components/watch-state-choice";
import { YearRange } from "@/features/library/components/year-range";
import type { BarProps } from "@/features/library/lib/listing-bar";
import { isFiltered, unfiltered } from "@/features/library/lib/title-listing";

const heading = "text-sm font-medium";

/** What the Filter button opens: genres, then Watch State, then years, and Clear filters. */
export function FilterPanel({ genres, ...bar }: BarProps & { genres: string[] }) {
  const clear = isFiltered(bar.listing) && (
    <div className="-mt-2.5 flex justify-end">
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
    // The popover keeps to the room under its button, so a short window scrolls the panel.
    <div className="flex min-h-0 flex-col gap-7.5 overflow-y-auto p-5">
      {genres.length > 0 && <GenreChips genres={genres} {...bar} />}
      <div className="flex flex-col gap-2.5">
        <p className={heading}>Watch State</p>
        <WatchStateChoice {...bar} />
      </div>
      <div className="flex flex-col gap-2.5">
        <p className={heading}>Year</p>
        <YearRange {...bar} />
      </div>
      {clear}
    </div>
  );
}
