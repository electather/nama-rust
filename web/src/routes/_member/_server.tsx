import { TransportProvider } from "@connectrpc/connect-query";
import { createFileRoute, redirect, retainSearchParams } from "@tanstack/react-router";

import { ServerLayout } from "@/app/server-layout";
import type { ServerToReach } from "@/features/servers/data/server-reach";
import { onlyReachedServer, serverToBrowse } from "@/features/servers/data/server-to-browse";
import { idParam } from "@/features/settings/lib/sections";

const noOtherServers: ServerToReach[] = [];

interface ServerSearch {
  server?: string | undefined;
}

/**
 * Pages of one Server's content. At the Hub, `?server=` names which of the Account's Servers, and
 * stays on every link between them (ADR 0105).
 */
export const Route = createFileRoute("/_member/_server")({
  validateSearch: (search: Record<string, unknown>): ServerSearch => ({
    server: idParam(search["server"]),
  }),
  search: { middlewares: [retainSearchParams(["server"])] },
  beforeLoad: async ({ context, location, search }) => {
    const { account } = context;
    if (!account) {
      return { here: undefined, elsewhere: noOtherServers };
    }
    if (search.server === undefined) {
      // A page's ids mean something only on one Server, so the Hub guesses only when there's one.
      const only = await onlyReachedServer({ ...context, account });
      if (!only) {
        throw redirect({ to: "/", replace: true });
      }
      throw redirect({
        to: location.pathname,
        search: { ...location.search, server: only.id },
        replace: true,
      });
    }
    return serverToBrowse({ ...context, account }, search.server);
  },
  component: ServerPages,
});

function ServerPages() {
  const { transport, account } = Route.useRouteContext();
  return (
    <TransportProvider transport={transport}>
      <ServerLayout atHub={account !== null} />
    </TransportProvider>
  );
}
