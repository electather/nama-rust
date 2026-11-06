import { TransportProvider } from "@connectrpc/connect-query";
import { createConnectTransport } from "@connectrpc/connect-web";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createRouter } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { routeTree } from "./routeTree.gen";

// The Server serves the app, so RPCs go to the page's own origin (ADR 0015).
const transport = createConnectTransport({ baseUrl: "/" });

export function getRouter() {
  const queryClient = new QueryClient();
  return createRouter({
    routeTree,
    scrollRestoration: true,
    defaultPreload: "intent",
    // TanStack Query caches the data, so the router shouldn't cache preloads on top of it.
    defaultPreloadStaleTime: 0,
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
