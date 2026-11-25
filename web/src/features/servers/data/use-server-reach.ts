import { useTransport } from "@connectrpc/connect-query";
import { useSuspenseQuery } from "@tanstack/react-query";
import { useRouteContext } from "@tanstack/react-router";

import { serverReachQuery } from "@/features/servers/data/server-reach";
import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";

/** What the page at the Hub can do with `server`, one of the signed-in Account's Servers. */
export function useServerReach(server: LinkedServer) {
  const hub = useTransport();
  const { account, serverAt } = useRouteContext({ from: "/_member" });
  const accountId = account?.id ?? "";
  return useSuspenseQuery(serverReachQuery({ hub, accountId, server, serverAt })).data;
}
