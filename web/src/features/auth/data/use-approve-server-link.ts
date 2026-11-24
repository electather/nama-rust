import { useMutation } from "@connectrpc/connect-query";

import { LinkService } from "@/gen/nama/hub/v1/link_pb";

/** Links the Server showing a code to the signed-in Account. */
export function useApproveServerLink() {
  return useMutation(LinkService.method.approveServerLink);
}
