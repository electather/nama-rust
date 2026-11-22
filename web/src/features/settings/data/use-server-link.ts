import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";

/** How often to look again while the Server shows a code, which is when it asks the Hub too. */
const WAITING_POLL_MS = 5000;

/** The query key of where Linking stands, to refetch after starting or Unlinking. */
export const serverLinkKey = createConnectQueryKey({
  schema: ServerService.method.getServerLink,
  cardinality: "finite",
});

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
