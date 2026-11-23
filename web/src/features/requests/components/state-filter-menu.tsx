import { ChevronDown } from "lucide-react";
import { Button } from "nama-ui/components/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "nama-ui/components/dropdown-menu";

import { type StateFilter, filterName, filterOrder } from "@/features/requests/lib/state-filter";
import { useModalContainer } from "@/shared/hooks/use-modal-container";

/** A "State: …" button whose menu offers each state filter, `chosen` being the one shown. */
export function StateFilterMenu({
  chosen,
  choose,
}: {
  chosen: StateFilter;
  choose: (filter: StateFilter) => void;
}) {
  // Its menu goes inside Settings' dialog, as only what's inside one shows over it.
  const [trigger, container] = useModalContainer();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button ref={trigger} variant="ghost" />}>
        State: {filterName(chosen)}
        <ChevronDown aria-hidden className="size-4" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" container={container}>
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
