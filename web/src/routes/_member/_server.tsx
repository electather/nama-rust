import { TransportProvider } from "@connectrpc/connect-query";
import { Outlet, createFileRoute, retainSearchParams } from "@tanstack/react-router";

import { serverToBrowse } from "@/features/servers/data/server-to-browse";
import { idParam } from "@/features/settings/lib/sections";
import { ConfirmItsYou } from "@/shared/components/confirm-its-you/confirm-its-you";

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
    if (account && search.server !== undefined) {
      return serverToBrowse({ ...context, account }, search.server);
    }
    return {};
  },
  component: ServerPages,
});

function ServerPages() {
  const { transport, account } = Route.useRouteContext();
  return (
    <TransportProvider transport={transport}>
      {/* A Server's own pages have theirs in `MemberLayout`; the Hub's have none. */}
      {account ? (
        <ConfirmItsYou>
          <Outlet />
        </ConfirmItsYou>
      ) : (
        <Outlet />
      )}
    </TransportProvider>
  );
}
