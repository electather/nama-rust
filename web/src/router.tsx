import { Code, type Transport } from "@connectrpc/connect";
import { TransportProvider } from "@connectrpc/connect-query";
import { createConnectTransport } from "@connectrpc/connect-web";
import { MutationCache, QueryCache, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRouter } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { RouteError } from "@/components/route-error";
import { signedInQuery, whoIsSignedIn } from "@/lib/access";
import { isCode } from "@/lib/connect-errors";

import { routeTree } from "./routeTree.gen";

// The Server serves the app, so RPCs go to the page's own origin (ADR 0015).
const sameOrigin = createConnectTransport({ baseUrl: "/" });

/** The app's router. Tests pass a fake `transport`. */
export function getRouter(transport: Transport = sameOrigin) {
  const queryClient = new QueryClient({
    queryCache: new QueryCache({ onError: recheckSession }),
    mutationCache: new MutationCache({ onError: recheckSession }),
    defaultOptions: { queries: { retry: retryIfUnreachable } },
  });
  const router = createRouter({
    routeTree,
    context: { queryClient, transport },
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

  // An `unauthenticated` may mean the session has ended, or may just be a wrong password. A
  // `permission_denied` may mean an Admin changed your Role. Asking again who is signed in tells
  // them apart. Only when that changed are the routes' access checks rerun: the `_member` layout
  // sends a signed-out visitor to /sign-in?redirect=…, and `_admin` a Member to "Admins only".
  function recheckSession(error: Error) {
    if (isCode(error, Code.Unauthenticated) || isCode(error, Code.PermissionDenied)) {
      void rerunAccessChecksIfChanged();
    }
  }
  async function rerunAccessChecksIfChanged() {
    const { queryKey } = signedInQuery(transport);
    const before = queryClient.getQueryData(queryKey)?.user;
    queryClient.removeQueries({ queryKey });
    const now = (await whoIsSignedIn({ queryClient, transport }))?.user;
    if (now?.id !== before?.id || now?.role !== before?.role) await router.invalidate();
  }

  return router;
}

// Retrying can't fix a refusal such as `unauthenticated`, and would hold up its redirect for
// seconds, so only a Server that couldn't be reached is tried again. A failed `fetch` arrives as
// `unknown`, the same as any other error that isn't from the Server.
function retryIfUnreachable(failures: number, error: Error) {
  return failures < 3 && (isCode(error, Code.Unavailable) || isCode(error, Code.Unknown));
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
