import { RadioSegment, RadioSegments } from "nama-ui/components/radio-group";

import { ANY, type BarProps } from "@/features/library/lib/listing-bar";
import { type Watched, watchedStates, withFilter } from "@/features/library/lib/title-listing";

const choices: Record<string, string> = { [ANY]: "All", ...watchedStates };

/** Which Watch State the grid keeps titles in, or all of them. */
export function WatchStateChoice({ listing, change }: BarProps) {
  return (
    <RadioSegments
      aria-label="Watch State"
      value={listing.watched ?? ANY}
      onValueChange={(value) => {
        const watched = Object.keys(watchedStates).find(
          (state): state is Watched => state === value,
        );
        change(withFilter(listing, { watched }));
      }}
    >
      {Object.entries(choices).map(([value, name]) => (
        <RadioSegment key={value} value={value}>
          {name}
        </RadioSegment>
      ))}
    </RadioSegments>
  );
}
