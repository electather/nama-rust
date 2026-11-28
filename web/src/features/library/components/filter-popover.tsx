import { ListFilter } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { Popover, PopoverContent, PopoverTrigger } from "nama-ui/components/popover";

import { FilterPanel } from "@/features/library/components/filter-panel";
import type { FilterProps } from "@/features/library/lib/listing-bar";
import { filterLabel, filterNames, isFiltered } from "@/features/library/lib/title-listing";

/**
 * The Filter button, saying what's on as "Sort: …" says its sort, lit while a filter is, and the
 * panel it opens. Its name lists every filter that's on, which its words cut to the first.
 */
export function FilterPopover(props: FilterProps) {
  const on = isFiltered(props.listing);
  return (
    <Popover>
      <PopoverTrigger
        render={<Button variant={on ? "secondary" : "ghost"} />}
        aria-label={on ? `Filter: ${filterNames(props.listing).join(", ")}` : "Filter"}
      >
        <ListFilter aria-hidden />
        {filterLabel(props.listing)}
      </PopoverTrigger>
      <PopoverContent side="bottom" width="list" flip>
        <FilterPanel {...props} />
      </PopoverContent>
    </Popover>
  );
}
