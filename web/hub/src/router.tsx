import type { Transport } from "@connectrpc/connect";
import { TransportProvider } from "@connectrpc/connect-query";
import { createConnectTransport } from "@connectrpc/connect-web";
import { RouteError } from "@hub/app/route-error";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRouter } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { routeTree } from "./routeTree.gen";

// The Hub serves the app, so RPCs go to the page's own origin (ADR 0015).
const sameOrigin = createConnectTransport({ baseUrl: "/" });

/** The Hub web app's router. Tests pass a fake `transport`. */
export function getRouter(transport: Transport = sameOrigin) {
  // A refusal such as "sign in first" is an answer, not a blip to retry.
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return createRouter({
    routeTree,
    context: { queryClient, transport },
    // TanStack Query caches the data, so the router shouldn't cache preloads on top of it.
    defaultPreloadStaleTime: 0,
    defaultErrorComponent: RouteError,
    Wrap: ({ children }: { children: ReactNode }) => (
      <TransportProvider transport={transport}>
        <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
      </TransportProvider>
    ),
  });
}

declare module "@tanstack/react-router" {
  interface Register {
    router: ReturnType<typeof getRouter>;
  }
}
