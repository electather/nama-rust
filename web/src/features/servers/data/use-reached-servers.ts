import { useTransport } from "@connectrpc/connect-query";
import { useQueries } from "@tanstack/react-query";
import { useRouteContext } from "@tanstack/react-router";

import { serverReachQuery } from "@/features/servers/data/server-reach";
import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";

/**
 * Each of `servers`, the signed-in Account's, signed in there, as it's reached. One not reached
 * yet, or that can't be, isn't listed, so a slow Server holds up none of the rest (ADR 0105).
 */
export function useReachedServers(servers: readonly LinkedServer[]) {
  const hub = useTransport();
  const { account, serverAt } = useRouteContext({ from: "/_member" });
  // Not suspended: a Server that's off fails only once its `fetch` gives up.
  return useQueries({
    queries: servers.map((server) =>
      serverReachQuery({ hub, accountId: account?.id ?? "", server, serverAt }),
    ),
    combine: (reaches) =>
      servers.flatMap((server, at) => {
        const reach = reaches[at]?.data;
        return reach?.kind === "signed-in" ? [{ server, ...reach }] : [];
      }),
  });
}
