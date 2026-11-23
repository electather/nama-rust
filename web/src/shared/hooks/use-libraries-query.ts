import { useQuery } from "@connectrpc/connect-query";

import { LibraryService } from "@/gen/nama/v1/library_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Server's Libraries without suspending, with whether they failed to load. */
export function useLibrariesQuery() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the app chrome and a Libraries setting show without the Libraries while they load or if they fail, so neither waits on them
  return useQuery(LibraryService.method.listLibraries, {}, { staleTime: loaderStaleTime });
}

/** The Server's Libraries once they've loaded, and none before then or if they failed. */
export function useLoadedLibraries() {
  return useLibrariesQuery().data?.libraries ?? [];
}
