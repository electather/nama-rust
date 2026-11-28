import { GenreChips } from "@/features/library/components/genre-chips";
import { UnmatchedChoice } from "@/features/library/components/unmatched-choice";
import { WatchStateChoice } from "@/features/library/components/watch-state-choice";
import type { FilterProps } from "@/features/library/lib/listing-bar";

const heading = "text-sm font-medium";

/** The Filter panel's choices: genres, then Watch State, then Not matched, scrolling in a short window. */
export function FilterChoices({ genres, unmatched, ...bar }: FilterProps) {
  return (
    // A short window scrolls the panel. While the popover eases to a new size it clips instead,
    // or a scrollbar coming and going narrows the chips, rewraps them and resizes it again.
    <div className="flex min-h-0 scrollbar-quiet flex-col gap-4 overflow-y-auto p-3 in-data-resizing:overflow-hidden">
      {genres.length > 0 && <GenreChips genres={genres} {...bar} />}
      <div className="flex flex-col gap-2">
        <p className={heading}>Watch State</p>
        <WatchStateChoice {...bar} />
      </div>
      <UnmatchedChoice count={unmatched} {...bar} />
    </div>
  );
}
