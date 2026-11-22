import type { Transport } from "@connectrpc/connect";
import type { QueryClient } from "@tanstack/react-query";

/** What every route's `beforeLoad` gets from the router. */
export interface RouterContext {
  queryClient: QueryClient;
  transport: Transport;
}
