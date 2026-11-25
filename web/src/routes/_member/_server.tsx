import { TransportProvider } from "@connectrpc/connect-query";
import { Outlet, createFileRoute, retainSearchParams } from "@tanstack/react-router";

import { connectedServer } from "@/features/servers/data/connected-server";
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
    if (context.account && search.server !== undefined) {
      return connectedServer(context, search.server);
    }
    return {};
  },
  component: ServerPages,
});

function ServerPages() {
  const { transport } = Route.useRouteContext();
  return (
    <TransportProvider transport={transport}>
      <Outlet />
    </TransportProvider>
  );
}
