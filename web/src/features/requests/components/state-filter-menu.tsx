import { ChevronDown } from "lucide-react";

import { type StateFilter, filterName, filterOrder } from "@/features/requests/lib/state-filter";
import { Button } from "@/shared/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/shared/ui/dropdown-menu";

/** A "State: …" button whose menu offers each state filter, `chosen` being the one shown. */
export function StateFilterMenu({
  chosen,
  choose,
}: {
  chosen: StateFilter;
  choose: (filter: StateFilter) => void;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="ghost" />}>
        State: {filterName(chosen)}
        <ChevronDown aria-hidden className="size-4" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start">
        {filterOrder.map((filter) => (
          <DropdownMenuItem
            key={filter}
            onClick={() => {
              choose(filter);
            }}
          >
            {filterName(filter)}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
