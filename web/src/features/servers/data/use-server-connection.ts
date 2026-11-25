import { useTransport } from "@connectrpc/connect-query";
import { useSuspenseQuery } from "@tanstack/react-query";
import { useRouteContext } from "@tanstack/react-router";

import { serverConnectionQuery } from "@/features/servers/data/server-connection";
import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";

/** How the page at the Hub reaches `server`, one of the Account's Servers. */
export function useServerConnection(server: LinkedServer) {
  const hub = useTransport();
  const serverAt = useRouteContext({ from: "__root__", select: (context) => context.serverAt });
  return useSuspenseQuery(serverConnectionQuery({ hub, server, serverAt })).data;
}
