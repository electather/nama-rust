import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";

import { LibraryService } from "@/gen/nama/v1/library_pb";

/** The Server's Libraries, for the Needs a Match page's loader to load before it shows. */
export function librariesQuery(transport: Transport) {
  return {
    ...createQueryOptions(LibraryService.method.listLibraries, {}, { transport }),
    staleTime: "static" as const,
  };
}

/** The Server's Libraries. */
export function useLibraries() {
  return useSuspenseQuery(LibraryService.method.listLibraries, {}).data.libraries;
}
