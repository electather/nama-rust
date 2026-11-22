import { useMutation, useQuery } from "@connectrpc/connect-query";
import { LinkService } from "@hub/gen/nama/hub/v1/link_pb";

/** The Server a sign-in with the Account is for, and the Account's User there, if it has one. */
export function useServerSignIn(id: string) {
  return useQuery(LinkService.method.getServerSignIn, { id }, { retry: false });
}

/** Approves the sign-in, then goes back to the Server, which finishes it. */
export function useApproveServerSignIn() {
  return useMutation(LinkService.method.approveServerSignIn, {
    onSuccess: ({ returnUrl }) => {
      globalThis.location.assign(returnUrl);
    },
  });
}
