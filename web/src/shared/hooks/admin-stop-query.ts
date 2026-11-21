import { queryOptions, skipToken } from "@tanstack/react-query";

import type { AdminStop } from "@/gen/nama/v1/event_pb";

/** Where an Admin's stop of the plan `planId` is kept, once one arrives. Only Events fill it. */
export function adminStopQuery(planId: bigint) {
  return queryOptions<AdminStop>({ queryKey: ["adminStop", String(planId)], queryFn: skipToken });
}
