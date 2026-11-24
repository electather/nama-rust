import { useMutation } from "@connectrpc/connect-query";

import { useSignInIfSessionEnded } from "@/features/auth/data/use-sign-in-if-session-ended";
import { LinkService } from "@/gen/nama/hub/v1/link_pb";

/**
 * Approves a sign-in to a Server, then goes back to the Server, which finishes it. When the
 * Account's session ended meanwhile, it signs in again and comes back here.
 */
export function useApproveServerSignIn() {
  return useMutation(LinkService.method.approveServerSignIn, {
    onSuccess: ({ returnUrl }) => {
      globalThis.location.assign(returnUrl);
    },
    onError: useSignInIfSessionEnded(),
  });
}
