import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey } from "@connectrpc/connect-query";

import { InboxService } from "@/gen/nama/v1/inbox_pb";

/** Every inbox query: the count and the list, to refresh when a notification may have come or been read. */
export function inboxKey(transport: Transport) {
  return createConnectQueryKey({ schema: InboxService, cardinality: undefined, transport });
}
