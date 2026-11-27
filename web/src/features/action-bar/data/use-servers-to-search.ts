import { useTransport } from "@connectrpc/connect-query";
import { useQuery } from "@tanstack/react-query";

import { useReachedServers } from "@/features/servers/data/use-reached-servers";
import { useHubAccount } from "@/shared/hooks/use-hub-account";
import { linkedServersQuery } from "@/shared/hooks/use-linked-servers";
import type { TitleServer } from "@/shared/lib/title-server";

/**
 * The Servers a search asks: this one, or at the Hub, each of the Account's Servers it has reached,
 * and whether any is still being reached.
 */
export function useServersToSearch(): { servers: TitleServer[]; reaching: boolean } {
  const transport = useTransport();
  const atHub = useHubAccount() !== null;
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- a Server's own web app has no Hub to ask, so it mustn't suspend there
  const linked = useQuery({
    ...linkedServersQuery(transport),
    enabled: atHub,
    select: ({ servers }) => servers,
  });
  const { reached, reaching } = useReachedServers(linked.data ?? []);
  if (!atHub) {
    return { servers: [{ server: undefined, transport }], reaching: false };
  }
  return {
    servers: reached.map((each) => ({ server: each.server, transport: each.transport })),
    reaching: linked.isPending || reaching,
  };
}
