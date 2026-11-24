import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey } from "@connectrpc/connect-query";
import type { Query } from "@tanstack/react-query";

import { ServerService } from "@/gen/nama/v1/server_pb";

/**
 * What names the Server `transport` talks to in a query key the app writes itself, second after
 * its name. It's the name connect-query gives the Server in its own keys, which it doesn't export.
 * The rule is in `web/src/AGENTS.md`, "Keys and paths name their Server".
 */
export function serverKey(transport: Transport) {
  return createConnectQueryKey({ schema: ServerService, transport, cardinality: undefined })[1]
    .transport;
}

/** Every query of the Server `transport` talks to: connect-query's and the app's own. */
export function serverQueries(transport: Transport) {
  const server = serverKey(transport);
  return {
    predicate: ({ queryKey: [, second] }: Query) =>
      second === server ||
      (typeof second === "object" &&
        second !== null &&
        "transport" in second &&
        second.transport === server),
  };
}
