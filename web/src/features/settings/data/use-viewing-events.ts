import { useSuspenseQuery } from "@connectrpc/connect-query";

import { ViewingEventService } from "@/gen/nama/hub/v1/viewing_pb";

/** Every one of the signed-in Account's Viewing Events, the latest started first, as the Hub sends them. */
export function useViewingEvents() {
  return useSuspenseQuery(ViewingEventService.method.listViewingEvents, {}).data;
}
