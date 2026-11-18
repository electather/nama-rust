import type { Transport } from "@connectrpc/connect";

import { type StateFilter, listedIn } from "@/features/requests/lib/state-filter";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { pagedQuery, usePaged } from "@/shared/hooks/use-paged";

/** `ListRequests`' first page of the Requests and Followed Shows in `filter`. */
function firstPageIn(filter: StateFilter | undefined) {
  return { filter: listedIn(filter), page: { token: "" } };
}

/** The first page of the Requests in `filter`, for the Requests page's loader to load before it shows. */
export function requestsQuery(filter: StateFilter | undefined, transport: Transport) {
  return {
    ...pagedQuery(RequestService.method.listRequests, firstPageIn(filter), transport),
    staleTime: "static" as const,
  };
}

/** The Requests and Followed Shows in `filter`, a page at a time. */
export function useRequests(filter: StateFilter) {
  return usePaged(RequestService.method.listRequests, firstPageIn(filter));
}
