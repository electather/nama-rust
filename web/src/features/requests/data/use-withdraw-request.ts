import type { DescMessage } from "@bufbuild/protobuf";
import { useMutation } from "@connectrpc/connect-query";

import type { Target } from "@/features/requests/lib/request-target";
import { useRefetchRequests } from "@/shared/hooks/use-refetch-requests";

/** The RPC that withdraws the Request or Followed Show `changes` names, then refetches the Requests. */
export function useWithdrawRequest(changes: Target["changes"]) {
  return useMutation<DescMessage, DescMessage>(changes.withdraw, useRefetchRequests());
}
