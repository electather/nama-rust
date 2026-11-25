import { useTransport } from "@connectrpc/connect-query";
import { useQueries } from "@tanstack/react-query";
import { useRouteContext } from "@tanstack/react-router";

import { type ServerReach, serverReachQuery } from "@/features/servers/data/server-reach";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { useLinkedServers } from "@/shared/hooks/use-linked-servers";
import { may } from "@/shared/lib/permissions";

/** One of the Account's Servers where its User is an Admin, signed in there. */
export type AdminServer = ReturnType<typeof useAdminServers>[number];

/**
 * The signed-in Account's Servers where its User is an Admin, each as it's reached. One not reached
 * yet, or that can't be, isn't listed, so a slow Server holds up none of the rest (ADR 0105).
 */
export function useAdminServers() {
  const hub = useTransport();
  const { account, serverAt } = useRouteContext({ from: "/_member" });
  const servers = useLinkedServers();
  // Not suspended: a Server that's off fails only once its `fetch` gives up.
  return useQueries({
    queries: servers.map((server) =>
      serverReachQuery({ hub, accountId: account?.id ?? "", server, serverAt }),
    ),
    combine: (reaches) =>
      servers.flatMap((server, at) => {
        const reach = reaches[at]?.data;
        return isAdminOn(reach) ? [{ server, ...reach }] : [];
      }),
  });
}

function isAdminOn(
  reach: ServerReach | undefined,
): reach is Extract<ServerReach, { kind: "signed-in" }> {
  return reach?.kind === "signed-in" && may(reach.signedIn.user, Permission.ADMIN);
}
