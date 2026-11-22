import { useSuspenseQuery } from "@connectrpc/connect-query";

import { RequestService } from "@/gen/nama/v1/request_pb";

/** How many Requests are Pending, for a Request manager's list to open on them. */
export function usePendingRequests() {
  return useSuspenseQuery(RequestService.method.countPendingRequests, {}).data.count;
}
