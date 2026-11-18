import { useQuery } from "@connectrpc/connect-query";

import { LibraryService } from "@/gen/nama/v1/library_pb";

/** The Server's Libraries for a Libraries setting, with whether they failed to load. */
export function useLibrariesToPick() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the setting shows its held Libraries while the list loads or if it fails, so the form never waits on it
  return useQuery(LibraryService.method.listLibraries, {});
}
