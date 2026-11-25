import { notFound } from "@tanstack/react-router";

import { serverConnectionQuery } from "@/features/servers/data/server-connection";
import type { RouterContext } from "@/shared/hooks/access";
import { linkedServersQuery } from "@/shared/hooks/use-linked-servers";
import type { ServerAt } from "@/shared/lib/server";

/**
 * The transport to the Account's Linked Server `linkedServerId`, and who it's signed in as there,
 * for the Server's pages at the Hub. Not found when the Account can't browse it.
 */
export async function connectedServer(
  context: RouterContext & { serverAt: ServerAt },
  linkedServerId: string,
) {
  const { queryClient, transport: hub, serverAt } = context;
  const { servers } = await queryClient.query(linkedServersQuery(hub));
  const server = servers.find(({ id }) => id === linkedServerId);
  if (!server) {
    throw notFound();
  }
  const connection = await queryClient.query(serverConnectionQuery({ hub, server, serverAt }));
  if (connection.kind !== "signed-in") {
    throw notFound();
  }
  return { transport: connection.transport, signedIn: connection.signedIn };
}
