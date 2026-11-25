import { TransportProvider } from "@connectrpc/connect-query";
import { createFileRoute, notFound, retainSearchParams } from "@tanstack/react-router";

import { ServerLayout } from "@/app/server-layout";
import { serverToBrowse } from "@/features/servers/data/server-to-browse";
import { idParam } from "@/features/settings/lib/sections";

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
  beforeLoad: ({ context, search }) => {
    const { account } = context;
    if (!account) {
      return {};
    }
    // The Hub has no content of its own to show on a Server's page.
    if (search.server === undefined) {
      throw notFound();
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
