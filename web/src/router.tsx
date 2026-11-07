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

  // Any `unauthenticated` may mean the session has ended, or may just be a wrong password. Asking
  // again who is signed in tells them apart. Only when nobody is are the routes' access checks
  // rerun, and the `_member` layout sends the visitor to /sign-in?redirect=….
  function recheckSession(error: Error) {
    if (isCode(error, Code.Unauthenticated)) void signInAgainIfSignedOut();
  }
  async function signInAgainIfSignedOut() {
    queryClient.removeQueries({ queryKey: signedInQuery(transport).queryKey });
    if ((await whoIsSignedIn({ queryClient, transport })) === null) await router.invalidate();
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
}
