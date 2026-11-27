import { FunnelX } from "lucide-react";

import { GenreChips } from "@/features/library/components/genre-chips";
import { WatchStateChoice } from "@/features/library/components/watch-state-choice";
import type { BarProps } from "@/features/library/lib/listing-bar";
import { isFiltered, unfiltered } from "@/features/library/lib/title-listing";
import { IconAction } from "@/shared/components/icon-action";

const heading = "text-sm font-medium";

/** What the Filter button opens: genres, then Watch State, with Clear filters in its corner. */
export function FilterPanel({ genres, ...bar }: BarProps & { genres: string[] }) {
  // In the corner, by the first heading, while there's something to clear.
  const clear = isFiltered(bar.listing) && (
    <div className="absolute inset-e-1.5 top-1.5">
      <IconAction
        label="Clear filters"
        aria-label="Clear filters"
        Icon={FunnelX}
        onClick={() => {
          bar.change(unfiltered(bar.listing));
        }}
      />
    </div>
  );
  return (
    // A short window scrolls the panel. While the popover eases to a new size it clips instead,
    // or a scrollbar coming and going narrows the chips, rewraps them and resizes it again.
    <div className="relative flex min-h-0 scrollbar-quiet flex-col gap-4 overflow-y-auto p-3 in-data-resizing:overflow-hidden">
      {clear}
      {genres.length > 0 && <GenreChips genres={genres} {...bar} />}
      <div className="flex flex-col gap-2">
        <p className={heading}>Watch State</p>
        <WatchStateChoice {...bar} />
      </div>
    </div>
  );
}
