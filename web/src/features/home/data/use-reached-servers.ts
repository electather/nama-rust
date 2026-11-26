import { useTransport } from "@connectrpc/connect-query";
import { useQueries } from "@tanstack/react-query";
import { useRouteContext } from "@tanstack/react-router";

import type { ReachedServer } from "@/features/home/lib/hub-rows";
import { serverReachQuery } from "@/features/servers/data/server-reach";
import { useLinkedServers } from "@/shared/hooks/use-linked-servers";

/** The Account's Servers the page at the Hub is signed in to, oldest first, as each answers. */
export function useReachedServers() {
  const hub = useTransport();
  const { account, serverAt } = useRouteContext({ from: "/_member" });
  const accountId = account?.id ?? "";
  const servers = useLinkedServers();
  // Not suspended, so a slow or unreachable Server holds up neither Home nor the rest.
  return useQueries({
    queries: servers.map((server) => serverReachQuery({ hub, accountId, server, serverAt })),
    combine: (reaches) =>
      servers.flatMap((server, at): ReachedServer[] => {
        const reach = reaches[at]?.data;
        return reach?.kind === "signed-in" ? [{ server, transport: reach.transport }] : [];
      }),
  });
}
