import { useQuery } from "@connectrpc/connect-query";

import { RequestService } from "@/gen/nama/v1/request_pb";

/** How many Requests are Pending, or 0 for a User who doesn't manage them, who isn't asked. */
export function usePendingRequestCount(managesRequests: boolean) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the app chrome shows on every page, so it mustn't wait on a count only Request managers see
  const { data } = useQuery(
    RequestService.method.countPendingRequests,
    {},
    { enabled: managesRequests },
  );
  return data?.count ?? 0;
}
