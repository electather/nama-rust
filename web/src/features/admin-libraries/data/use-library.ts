import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { LibraryService } from "@/gen/nama/v1/library_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The query key of the Libraries, and of each one, to refetch after a change to them. */
export function librariesKey(transport: Transport) {
  return createConnectQueryKey({
    schema: LibraryService,
    cardinality: "finite",
    transport,
  });
}

/** The Library `id`, with what its last scan found, which a finished scan refetches. */
export function useLibrary(id: bigint) {
  return useSuspenseQuery(LibraryService.method.getLibrary, { id }, { staleTime: loaderStaleTime })
    .data.library;
}
