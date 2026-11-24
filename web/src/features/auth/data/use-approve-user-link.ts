import { useMutation } from "@connectrpc/connect-query";

import { LinkService } from "@/gen/nama/hub/v1/link_pb";

/** Links the User whose Server shows them a code to the signed-in Account. */
export function useApproveUserLink() {
  return useMutation(LinkService.method.approveUserLink);
}
