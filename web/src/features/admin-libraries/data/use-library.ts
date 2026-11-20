import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { LibraryService } from "@/gen/nama/v1/library_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The query key of the Libraries, and of each one, to refetch after a change to them. */
export const librariesKey = createConnectQueryKey({
  schema: LibraryService,
  cardinality: "finite",
});

/** The Library `id`, which the route's loader loads, or undefined once it's gone. */
export function useLibrary(id: bigint) {
  return useSuspenseQuery(LibraryService.method.getLibrary, { id }, { staleTime: loaderStaleTime })
    .data.library;
}
