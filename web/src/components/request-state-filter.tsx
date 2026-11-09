import { createQueryOptions } from "@connectrpc/connect-query";
import { ChevronDown } from "lucide-react";

import { requestStateName } from "@/components/request-state";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { RequestService, RequestState } from "@/gen/nama/v1/request_pb";
import { Permission, type User } from "@/gen/nama/v1/sign_in_pb";
import type { RouterContext } from "@/lib/access";
import { may } from "@/lib/permissions";

/** The Requests page's `state` search parameter values, in the order the filter offers them. */
const stateFilters = {
  all: RequestState.UNSPECIFIED,
  pending: RequestState.PENDING,
  approved: RequestState.APPROVED,
  processing: RequestState.PROCESSING,
  failed: RequestState.FAILED,
  "partially-available": RequestState.PARTIALLY_AVAILABLE,
  available: RequestState.AVAILABLE,
  declined: RequestState.DECLINED,
};

/** A state to list Requests and Followed Shows in, as the Requests page's URL names it. */
export type StateFilter = keyof typeof stateFilters;

/** Whether `value` names a state filter, so an unknown one in a URL counts as none. */
export function isStateFilter(value: unknown): value is StateFilter {
  return typeof value === "string" && Object.hasOwn(stateFilters, value);
}

// Every key passes: the filter only narrows `Object.keys`'s `string[]` to the filters' type.
const filterOrder = Object.keys(stateFilters).filter((key) => isStateFilter(key));

/** `filter` in the Glossary's words. */
export function filterName(filter: StateFilter) {
  return filter === "all" ? "All" : requestStateName(stateFilters[filter]);
}

/** The filter the Requests page opens on: Pending for a Request manager while anything is, else All. */
export async function defaultFilter(
  { queryClient, transport }: RouterContext,
  user: User | undefined,
): Promise<StateFilter> {
  if (!may(user, Permission.MANAGE_REQUESTS)) return "all";
  // Not `staleTime: "static"` like other route queries: a count cached from earlier could be out of
  // date and open the page on the wrong filter.
  const { count } = await queryClient.query(
    createQueryOptions(RequestService.method.countPendingRequests, {}, { transport }),
  );
  return count > 0 ? "pending" : "all";
}

/** `ListRequests`'s state for `filter`, where no filter means All. */
export function listedIn(filter: StateFilter | undefined) {
  return { state: stateFilters[filter ?? "all"] };
}

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
