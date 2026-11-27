import { ListFilter } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { Popover, PopoverContent, PopoverTrigger } from "nama-ui/components/popover";

import { FilterPanel } from "@/features/library/components/filter-panel";
import type { BarProps } from "@/features/library/lib/listing-bar";
import { filterCount } from "@/features/library/lib/title-listing";

/** The Filter button, lit with how many filters are on, and the panel it opens. */
export function FilterPopover(props: BarProps & { genres: string[] }) {
  const count = filterCount(props.listing);
  return (
    <Popover>
      <PopoverTrigger
        render={<Button variant={count === 0 ? "ghost" : "secondary"} />}
        aria-label={count === 0 ? "Filter" : `Filter, ${count} on`}
      >
        <ListFilter aria-hidden />
        Filter
        {count > 0 && <span className="tabular-nums">{count}</span>}
      </PopoverTrigger>
      <PopoverContent side="bottom" width="text" flip>
        <FilterPanel {...props} />
      </PopoverContent>
    </Popover>
  );
}
