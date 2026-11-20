import { useQuery } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** This Server's name, once it has loaded. */
export function useServerName() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the form shows at once, under a heading without the name until it arrives
  return useQuery(ServerService.method.getServerInfo, {}, { staleTime: loaderStaleTime }).data
    ?.name;
}
