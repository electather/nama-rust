import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { WAITING_POLL_MS } from "@/features/settings/lib/server-link";
import { ServerService } from "@/gen/nama/v1/server_pb";

/** The query key of where Linking stands, to refetch after starting or Unlinking. */
export function serverLinkKey(transport: Transport) {
  return createConnectQueryKey({
    schema: ServerService.method.getServerLink,
    cardinality: "finite",
    transport,
  });
}

/**
 * Where Linking the Server stands, or `undefined` without an enabled Plugin that links it, such
 * as the Hub Plugin. Looked at again while a code waits to be entered.
 */
export function useServerLink() {
  return useSuspenseQuery(
    ServerService.method.getServerLink,
    {},
    {
      refetchInterval: (query) =>
        (query.state.data?.link?.code ?? "") === "" ? false : WAITING_POLL_MS,
      // The code is entered on the Hub, usually in another tab, and the Server collects its
      // credential only when asked: waiting for this tab to come back could outlast the code.
      refetchIntervalInBackground: true,
    },
  ).data.link;
}
