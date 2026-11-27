import type { Transport } from "@connectrpc/connect";
import { TransportProvider } from "@connectrpc/connect-query";
import { QueryClient, QueryClientProvider, hashKey } from "@tanstack/react-query";
import {
  RouterProvider,
  createMemoryHistory,
  createRootRouteWithContext,
  createRoute,
  createRouter,
} from "@tanstack/react-router";
import { render, waitFor } from "@testing-library/react";

import {
  type IdentityChange,
  useIdentityChange,
  useSignInIfSessionEnded,
} from "@/features/auth/data/use-identity-change";
import type { RouterContext } from "@/shared/hooks/access";
import type { ServerAt } from "@/shared/lib/server";

/** Where a change runs: the Server its page talks to, the Hub the app runs at, if any, and theirs. */
export interface ChangeAt {
  server: Transport;
  /** The page's own origin's: the Server's, or else the Hub's. */
  hub: Transport;
  serverAt: ServerAt;
}

/** The sessions this browser keeps on Linked Servers, by key. */
export function keptSessionKeys() {
  return Object.keys(localStorage).filter((key) => key.startsWith("nama:server-session:"));
}

/**
 * Opens a page at `/start`, after `/before`, that makes `change` at `at`, noting in `log` when the
 * cache is cleared and where the page goes, with whether sessions were kept then. Returns the
 * change, the page's `onError` for a Hub call, the router, and the cache, which holds something
 * from before.
 */
export async function openIdentityChange<T>(
  change: IdentityChange<T>,
  at: ChangeAt,
  log: string[],
) {
  const queryClient = new QueryClient();
  queryClient.setQueryData(fromBefore, "theirs");
  queryClient.getQueryCache().subscribe(({ type, query }) => {
    if (type === "removed" && query.queryHash === hashKey(fromBefore)) {
      log.push("cache cleared");
    }
  });
  const made: {
    change?: ReturnType<typeof useIdentityChange<T>>;
    onHubError?: ReturnType<typeof useSignInIfSessionEnded>;
  } = {};
  function Start() {
    made.change = useIdentityChange(change);
    made.onHubError = useSignInIfSessionEnded();
    return null;
  }
  const root = createRootRouteWithContext<RouterContext & { serverAt: ServerAt }>()();
  const pages = ["/", "/next", "/history", "/sign-in", "/switch-user"].map((path) =>
    createRoute({ getParentRoute: () => root, path }),
  );
  const start = createRoute({ getParentRoute: () => root, path: "/start", component: Start });
  const router = createRouter({
    routeTree: root.addChildren([...pages, start]),
    history: createMemoryHistory({ initialEntries: ["/before", "/start"], initialIndex: 1 }),
    context: { queryClient, transport: at.hub, serverAt: at.serverAt },
  });
  router.history.subscribe(({ location, action }) => {
    const went = action.type === "REPLACE" ? "replaced with" : "went to";
    const kept = keptSessionKeys().length > 0 ? " (sessions kept)" : "";
    log.push(`${went} ${location.href}${kept}`);
  });
  render(
    <TransportProvider transport={at.server}>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </TransportProvider>,
  );
  const ready = await waitFor(() => {
    if (made.change === undefined || made.onHubError === undefined) {
      throw new Error("not yet started");
    }
    return { change: made.change, onHubError: made.onHubError };
  });
  return { ...ready, router, queryClient };
}

const fromBefore = ["from before"];

/** What the cache of `openIdentityChange` holds from before, until a change clears it. */
export function cachedFromBefore(queryClient: QueryClient) {
  return queryClient.getQueryData(fromBefore);
}
