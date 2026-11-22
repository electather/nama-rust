import { type StateFilter, listedIn } from "@/features/requests/lib/state-filter";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { usePaged } from "@/shared/hooks/use-paged";

/** `ListRequests`' first page of the Requests and Followed Shows in `filter`. */
function firstPageIn(filter: StateFilter) {
  return { filter: listedIn(filter), page: { token: "" } };
}

/** The Requests and Followed Shows in `filter`, a page at a time. */
export function useRequests(filter: StateFilter) {
  return usePaged(RequestService.method.listRequests, firstPageIn(filter));
}
