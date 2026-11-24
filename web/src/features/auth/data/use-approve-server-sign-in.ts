import { useMutation } from "@connectrpc/connect-query";

import { LinkService } from "@/gen/nama/hub/v1/link_pb";

/** Approves a sign-in to a Server, then goes back to the Server, which finishes it. */
export function useApproveServerSignIn() {
  return useMutation(LinkService.method.approveServerSignIn, {
    onSuccess: ({ returnUrl }) => {
      globalThis.location.assign(returnUrl);
    },
  });
}
