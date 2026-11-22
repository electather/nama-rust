import { useSuspenseQuery } from "@connectrpc/connect-query";

import { LibraryService } from "@/gen/nama/v1/library_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Server's Libraries. */
export function useLibraries() {
  return useSuspenseQuery(LibraryService.method.listLibraries, {}, { staleTime: loaderStaleTime })
    .data.libraries;
}
