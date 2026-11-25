import { useSuspenseQuery } from "@connectrpc/connect-query";

import { ViewingEventService } from "@/gen/nama/hub/v1/viewing_pb";

/** Whether the signed-in Account's finished Movies and Watchlists count in the pooled counts. */
export function usePooling() {
  return useSuspenseQuery(ViewingEventService.method.getPooling, {}).data.pooling;
}
