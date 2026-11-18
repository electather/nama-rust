import type { Transport } from "@connectrpc/connect";
import { createQueryOptions } from "@connectrpc/connect-query";

import { RequestService } from "@/gen/nama/v1/request_pb";

/** How many Requests are Pending, not `staleTime: "static"`, so a count cached earlier can't be out of date. */
export function pendingCountQuery(transport: Transport) {
  return createQueryOptions(RequestService.method.countPendingRequests, {}, { transport });
}
