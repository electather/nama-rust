import { createQueryOptions, useTransport } from "@connectrpc/connect-query";
import { useQueries } from "@tanstack/react-query";
import { useRouteContext } from "@tanstack/react-router";

import { serverReachQuery } from "@/features/servers/data/server-reach";
import { mergedWatchlist } from "@/features/watchlist/lib/merged-watchlist";
import { WatchlistService } from "@/gen/nama/v1/watchlist_pb";
import { useLinkedServers } from "@/shared/hooks/use-linked-servers";

/**
 * At the Hub, the Watchlists of the Account's Servers merged, each title once, as each Server
 * answers, and whether any has yet to.
 */
export function useHubWatchlist() {
  const servers = useLinkedServers();
  const hub = useTransport();
  const { account, serverAt } = useRouteContext({ from: "/_member" });
  const accountId = account?.id ?? "";
  // Not suspended, so a slow or unreachable Server holds up neither the page nor the rest.
  const reached = useQueries({
    queries: servers.map((server) => serverReachQuery({ hub, accountId, server, serverAt })),
    combine: (reaches) => ({
      pending: reaches.some(({ isPending }) => isPending),
      signedIn: servers.flatMap((server, at) => {
        const reach = reaches[at]?.data;
        return reach?.kind === "signed-in" ? [{ server, transport: reach.transport }] : [];
      }),
    }),
  });
  return useQueries({
    queries: reached.signedIn.map(({ transport }) =>
      createQueryOptions(WatchlistService.method.listWatchlist, {}, { transport }),
    ),
    combine: (lists) => ({
      loading: reached.pending || lists.some(({ isPending }) => isPending),
      entries: mergedWatchlist(
        reached.signedIn.flatMap(({ server, transport }, at) => {
          const titles = lists[at]?.data?.titles;
          return titles ? [{ server, transport, titles }] : [];
        }),
      ),
    }),
  });
}
