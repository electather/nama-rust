import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";

import { LibraryService } from "@/gen/nama/v1/library_pb";
import type { RouterContext } from "@/shared/hooks/access";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Server's Libraries, for a route's loader to fetch before its page shows. */
export function librariesQuery(transport: Transport) {
  return {
    ...createQueryOptions(LibraryService.method.listLibraries, {}, { transport }),
    staleTime: "static" as const,
  };
}

/**
 * Loads the Libraries for a title page's link to its Library. A loader doesn't await it, so the
 * title's own call goes out beside it rather than after it.
 */
export async function prefetchLibraries({ queryClient, transport }: RouterContext) {
  try {
    await queryClient.query(librariesQuery(transport));
  } catch {
    // The title page shows without its Library then.
  }
}

/** The Libraries of the Server `transport` reaches, by default the page's own. */
export function useLibraries(transport?: Transport) {
  return useSuspenseQuery(
    LibraryService.method.listLibraries,
    {},
    { staleTime: loaderStaleTime, ...(transport && { transport }) },
  ).data.libraries;
}
