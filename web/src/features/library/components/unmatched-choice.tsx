import { Button } from "nama-ui/components/button";

import type { BarProps } from "@/features/library/lib/listing-bar";
import { isUnmatched, unfiltered } from "@/features/library/lib/title-listing";

/**
 * A chip that lists only the Library's Unmatched Items, as tiles, turning the title filters off.
 * Left out when it has none (`count`) and the chip is off.
 */
export function UnmatchedChoice({ listing, change, count }: BarProps & { count: number }) {
  const on = isUnmatched(listing);
  if (!on && count === 0) {
    return null;
  }
  return (
    <div className="flex">
      <Button
        variant="secondary"
        aria-pressed={on}
        onClick={() => {
          change(on ? unfiltered(listing) : { ...unfiltered(listing), unmatched: true });
        }}
      >
        Not matched
        <span className="ms-1 font-normal text-muted-foreground">{count.toLocaleString()}</span>
      </Button>
    </div>
  );
}
