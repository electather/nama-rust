import { RequestState } from "@/gen/nama/v1/request_pb";
import { requestStateName } from "@/shared/components/request-state/request-states";

/** The states Requests are listed in, in the order the filter offers them. */
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

/** A state to list Requests and Followed Shows in. */
export type StateFilter = keyof typeof stateFilters;

function isStateFilter(value: string): value is StateFilter {
  return Object.hasOwn(stateFilters, value);
}

// Every key passes: the filter only narrows `Object.keys`'s `string[]` to the filters' type.
/** Every state filter, in the order the filter offers them. */
export const filterOrder = Object.keys(stateFilters).filter((key) => isStateFilter(key));

/** `filter` in the Glossary's words. */
export function filterName(filter: StateFilter) {
  return filter === "all" ? "All" : requestStateName(stateFilters[filter]);
}

/** `ListRequests`'s `RequestFilter` for `filter`. */
export function listedIn(filter: StateFilter) {
  return { state: stateFilters[filter] };
}
