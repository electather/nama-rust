import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";

import { LibraryService } from "@/gen/nama/v1/library_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Server's Libraries, for a route's loader to fetch before its page shows. */
export function librariesQuery(transport: Transport) {
  return {
    ...createQueryOptions(LibraryService.method.listLibraries, {}, { transport }),
    staleTime: "static" as const,
  };
}

/** The Server's Libraries. */
export function useLibraries() {
  return useSuspenseQuery(LibraryService.method.listLibraries, {}, { staleTime: loaderStaleTime })
    .data.libraries;
}
