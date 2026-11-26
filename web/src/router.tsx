import { Code, type Transport } from "@connectrpc/connect";
import { TransportProvider } from "@connectrpc/connect-query";
import { MutationCache, QueryCache, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRouter } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { RouteError } from "@/app/route-error/route-error";
import { isAtHub, signedInQuery, whoIsSignedIn } from "@/shared/hooks/access";
import { isCode, retryIfUnreachable } from "@/shared/lib/connect-errors";
import { traceCalls, traceRouteChanges } from "@/shared/lib/diagnostics";
import { type ServerAt, serverTransport } from "@/shared/lib/server";

// oxlint-disable-next-line import/max-dependencies -- the router puts together the transport, the query cache, the routes and their Diagnostics
import { routeTree } from "./routeTree.gen";

// The Server serves the app, so RPCs go to the page's own origin (ADR 0015).
const sameOrigin = serverTransport("", [traceCalls]);

/**
 * The app's router. Tests pass a fake `transport`, and at the Hub a fake `serverAt`, which reaches
 * each of the Account's Servers.
 */
export function getRouter(transport: Transport = sameOrigin, serverAt: ServerAt = serverTransport) {
  const queryClient = new QueryClient({
    queryCache: new QueryCache({ onError: recheckSession }),
    mutationCache: new MutationCache({ onError: recheckSession }),
    defaultOptions: { queries: { retry: retryIfUnreachable } },
  });
  const router = createRouter({
    routeTree,
    context: { queryClient, transport, serverAt },
    scrollRestoration: true,
    defaultPreload: "intent",
    // TanStack Query caches the data, so the router shouldn't cache preloads on top of it.
    defaultPreloadStaleTime: 0,
    defaultErrorComponent: RouteError,
    Wrap: ({ children }: { children: ReactNode }) => (
      <TransportProvider transport={transport}>
        <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
      </TransportProvider>
    ),
  });
  traceRouteChanges(router);

  // An `unauthenticated` may mean the session has ended, or may just be a wrong password. A
  // `permission_denied` may mean an Admin changed your Permissions. Asking again who is signed in
  // tells them apart. Only when that changed are the routes' access checks rerun: the `_member`
  // layout sends a signed-out visitor to /sign-in?redirect=…, and works out again where the top
  // bar's Admin opens Settings.
  function recheckSession(error: Error) {
    if (isCode(error, Code.Unauthenticated) || isCode(error, Code.PermissionDenied)) {
      void rerunAccessChecksIfChanged();
    }
  }
  async function rerunAccessChecksIfChanged() {
    // At the Hub no Server User is signed in; a Hub page handles its Account's ended session.
    if (await isAtHub({ queryClient, transport })) {
      return;
    }
    const { queryKey } = signedInQuery(transport);
    const before = queryClient.getQueryData(queryKey)?.user;
    queryClient.removeQueries({ queryKey });
    const now = (await whoIsSignedIn({ queryClient, transport }))?.user;
    // The Server lists Permissions in one order, so the same set joins the same.
    const changed =
      now?.id !== before?.id || now?.permissions.join(",") !== before?.permissions.join(",");
    if (changed) {
      await router.invalidate();
    }
  }

  return router;
}

declare module "@tanstack/react-router" {
  interface Register {
    router: ReturnType<typeof getRouter>;
  }
  interface StaticDataRouteOption {
    /** The page fills the screen, without the app chrome, such as the player. */
    fullScreen?: boolean;
  }
}
