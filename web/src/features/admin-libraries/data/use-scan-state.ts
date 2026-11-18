import { useQuery } from "@tanstack/react-query";

import { scanQuery } from "@/lib/events";

/** The latest scan Event about the Library `libraryId`, if one arrived since the page opened. */
export function useScanState(libraryId: bigint) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- only Events fill it, so there's no fetch to wait for
  return useQuery(scanQuery(libraryId)).data;
}
