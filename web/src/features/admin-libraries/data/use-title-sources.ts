import { useQuery } from "@connectrpc/connect-query";

import { LibraryService } from "@/gen/nama/v1/library_pb";

/** The title sources the Server's Plugins offer, once they've loaded. */
export function useTitleSources() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the field shows the stored sources at once, and marks one missing only once the list arrives
  return useQuery(LibraryService.method.listTitleSources, {}).data;
}
