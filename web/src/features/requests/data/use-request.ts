import { useSuspenseQuery } from "@connectrpc/connect-query";

import { RequestService } from "@/gen/nama/v1/request_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Request `id`, which the query leaves in place when a refetch finds it gone. */
export function useRequest(id: bigint) {
  return useSuspenseQuery(RequestService.method.getRequest, { id }, { staleTime: loaderStaleTime });
}
