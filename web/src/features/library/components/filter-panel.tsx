import { Button } from "nama-ui/components/button";

import { GenreChips } from "@/features/library/components/genre-chips";
import { WatchStateChoice } from "@/features/library/components/watch-state-choice";
import { YearRange } from "@/features/library/components/year-range";
import type { BarProps } from "@/features/library/lib/listing-bar";
import { isFiltered, unfiltered } from "@/features/library/lib/title-listing";

/** What the Filter button opens: genres, then Watch State, then years, and Clear filters. */
export function FilterPanel({ genres, ...bar }: BarProps & { genres: string[] }) {
  const heading = "text-xs text-muted-foreground";
  return (
    // The popover keeps to the room under its button, so a short window scrolls the panel.
    <div className="flex min-h-0 flex-col gap-2 overflow-y-auto p-3">
      {genres.length > 0 && <GenreChips genres={genres} {...bar} />}
      <p className={genres.length > 0 ? `mt-2 ${heading}` : heading}>Watch State</p>
      <WatchStateChoice {...bar} />
      <p className={`mt-2 ${heading}`}>Year</p>
      <YearRange {...bar} />
      {isFiltered(bar.listing) && (
        <div className="mt-2">
          <Button
            variant="ghost"
            onClick={() => {
              bar.change(unfiltered(bar.listing));
            }}
          >
            Clear filters
          </Button>
        </div>
      )}
    </div>
  );
}
