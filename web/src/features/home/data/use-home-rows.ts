import { useQuery } from "@connectrpc/connect-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";

/** The Home rows Plugins give, once they have loaded. */
export function useHomeRows() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- Home never waits on a Plugin's rows
  return useQuery(BrowseService.method.listHomeRows, {}).data?.rows;
}
