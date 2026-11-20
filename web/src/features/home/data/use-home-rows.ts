import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** Home's layout: its rows, which the route's loader waits on before Home shows. */
export function homeRowsQuery(transport: Transport) {
  return {
    ...createQueryOptions(BrowseService.method.listHomeRows, {}, { transport }),
    staleTime: "static" as const,
  };
}

/** The Home rows, the Kernel's and the Plugins', in the order Home shows them. */
export function useHomeRows() {
  return useSuspenseQuery(BrowseService.method.listHomeRows, {}, { staleTime: loaderStaleTime })
    .data.rows;
}
