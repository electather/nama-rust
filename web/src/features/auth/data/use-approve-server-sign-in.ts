import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useRouter } from "@tanstack/react-router";

import { hubRefusalReason } from "@/features/auth/lib/hub-errors";
import { LinkService } from "@/gen/nama/hub/v1/link_pb";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";
import { forgetSignedInAccount } from "@/shared/hooks/access";

/**
 * Approves a sign-in to a Server, then goes back to the Server, which finishes it. When the
 * Account's session ended meanwhile, it signs in again and comes back here.
 */
export function useApproveServerSignIn() {
  const queryClient = useQueryClient();
  const router = useRouter();
  return useMutation(LinkService.method.approveServerSignIn, {
    onSuccess: ({ returnUrl }) => {
      globalThis.location.assign(returnUrl);
    },
    // `src/router.tsx` leaves an ended session to the Hub's page: rerunning `_member`'s access
    // check sends the Account to sign in, then back to this page.
    onError: async (error) => {
      if (hubRefusalReason(error) === RefusalReason.SIGN_IN_REQUIRED) {
        forgetSignedInAccount(queryClient);
        await router.invalidate();
      }
    },
  });
}
