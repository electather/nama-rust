import { FunnelX } from "lucide-react";

import type { BarProps } from "@/features/library/lib/listing-bar";
import { isFiltered, unfiltered } from "@/features/library/lib/title-listing";
import { IconAction } from "@/shared/components/icon-action";

/**
 * The Filter panel's Clear filters, which turns every filter off, while one is on.
 */
export function ClearFilters({ listing, change }: BarProps) {
  if (!isFiltered(listing)) {
    return null;
  }
  return (
    <IconAction
      label="Clear filters"
      aria-label="Clear filters"
      Icon={FunnelX}
      onClick={() => {
        change(unfiltered(listing));
      }}
    />
  );
}
