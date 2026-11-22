import { ArrowDownWideNarrow, ArrowUpNarrowWide } from "lucide-react";
import { Button } from "nama-ui/components/button";

import { ChoiceMenu } from "@/features/library/components/choice-menu";
import type { BarProps } from "@/features/library/lib/listing-bar";
import { type Sort, isDescending, sorts } from "@/features/library/lib/title-listing";

/** The sort menu and the button that flips its direction. */
export function SortControls({ listing, change }: BarProps) {
  const descending = isDescending(listing);
  return (
    <>
      <ChoiceMenu
        label="Sort"
        choices={sorts}
        chosen={listing.sort ?? "title"}
        choose={(sort: Sort) => {
          // A new sort runs its own way, so the direction from the last one goes.
          change({ ...listing, sort, order: undefined });
        }}
      />
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={descending ? "Sort ascending" : "Sort descending"}
        onClick={() => {
          change({ ...listing, order: descending ? "asc" : "desc" });
        }}
      >
        {descending ? <ArrowDownWideNarrow aria-hidden /> : <ArrowUpNarrowWide aria-hidden />}
      </Button>
    </>
  );
}
