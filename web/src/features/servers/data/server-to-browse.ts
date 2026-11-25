import { notFound } from "@tanstack/react-router";

import { type ServerToReach, serverReachQuery } from "@/features/servers/data/server-reach";
import type { Account } from "@/gen/nama/hub/v1/account_pb";
import type { LinkedServer } from "@/gen/nama/hub/v1/link_pb";
import type { RouterContext } from "@/shared/hooks/access";
import { linkedServersQuery } from "@/shared/hooks/use-linked-servers";
import type { ServerAt } from "@/shared/lib/server";

/**
 * The transport to the Account's Linked Server `linkedServerId`, who it's signed in as there, that
 * Server as `here` and the Account's others as `elsewhere`, for the Server's pages at the Hub. Not
 * found when the Account can't browse it.
 */
export async function serverToBrowse(
  context: RouterContext & { serverAt: ServerAt; account: Account },
  linkedServerId: string,
) {
  const { queryClient, transport: hub, serverAt, account } = context;
  const { servers } = await queryClient.query(linkedServersQuery(hub));
  const here = servers.find(({ id }) => id === linkedServerId);
  if (!here) {
    throw notFound();
  }
  const reach = (server: LinkedServer): ServerToReach => ({
    hub,
    accountId: account.id,
    server,
    serverAt,
  });
  const reached = await queryClient.query(serverReachQuery(reach(here)));
  if (reached.kind !== "signed-in") {
    throw notFound();
  }
  const elsewhere = servers.filter((server) => server !== here).map((server) => reach(server));
  return { transport: reached.transport, signedIn: reached.signedIn, here, elsewhere };
}
