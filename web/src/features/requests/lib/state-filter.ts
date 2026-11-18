import { requestStateName } from "@/components/request-state";
import { RequestState } from "@/gen/nama/v1/request_pb";

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
/** Every state filter, in the order the filter offers them. */
export const filterOrder = Object.keys(stateFilters).filter((key) => isStateFilter(key));

/** `filter` in the Glossary's words. */
export function filterName(filter: StateFilter) {
  return filter === "all" ? "All" : requestStateName(stateFilters[filter]);
}

/** `ListRequests`'s `RequestFilter` for `filter`, where no filter means All. */
export function listedIn(filter: StateFilter | undefined) {
  return { state: stateFilters[filter ?? "all"] };
}
