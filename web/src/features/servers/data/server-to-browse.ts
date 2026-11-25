import { notFound } from "@tanstack/react-router";

import { serverReachQuery } from "@/features/servers/data/server-reach";
import type { Account } from "@/gen/nama/hub/v1/account_pb";
import type { RouterContext } from "@/shared/hooks/access";
import { linkedServersQuery } from "@/shared/hooks/use-linked-servers";
import type { ServerAt } from "@/shared/lib/server";

/**
 * The transport to the Account's Linked Server `linkedServerId`, and who it's signed in as there,
 * for the Server's pages at the Hub. Not found when the Account can't browse it.
 */
export async function serverToBrowse(
  context: RouterContext & { serverAt: ServerAt; account: Account },
  linkedServerId: string,
) {
  const { queryClient, transport: hub, serverAt, account } = context;
  const { servers } = await queryClient.query(linkedServersQuery(hub));
  const server = servers.find(({ id }) => id === linkedServerId);
  if (!server) {
    throw notFound();
  }
  const reach = { hub, accountId: account.id, server, serverAt };
  const reached = await queryClient.query(serverReachQuery(reach));
  if (reached.kind !== "signed-in") {
    throw notFound();
  }
  return { transport: reached.transport, signedIn: reached.signedIn };
}
