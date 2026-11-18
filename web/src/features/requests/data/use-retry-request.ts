import { useMutation } from "@connectrpc/connect-query";

import { RequestService } from "@/gen/nama/v1/request_pb";
import { useRefetchRequests } from "@/shared/hooks/use-refetch-requests";

/** Hands a Failed Request to its Fulfiller again, then refetches the Requests. */
export function useRetryRequest() {
  return useMutation(RequestService.method.retryRequest, useRefetchRequests());
}
