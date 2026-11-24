import type { Transport } from "@connectrpc/connect";
import { queryOptions, skipToken } from "@tanstack/react-query";

import type { AdminStop } from "@/gen/nama/v1/event_pb";
import { serverKey } from "@/shared/hooks/server-key";

/**
 * Where an Admin's stop of the plan `planId` on the Server `transport` talks to is kept, once one
 * arrives. Only Events fill it.
 */
export function adminStopQuery(transport: Transport, planId: bigint) {
  return queryOptions<AdminStop>({
    queryKey: ["adminStop", serverKey(transport), String(planId)],
    queryFn: skipToken,
  });
}
