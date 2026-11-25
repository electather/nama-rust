import { useQueryClient } from "@tanstack/react-query";
import { useRouter } from "@tanstack/react-router";

import { hubRefusalReason } from "@/features/auth/lib/hub-errors";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";
import { forgetSignedInAccount } from "@/shared/hooks/access";

/**
 * An `onError` for a call to the Hub that sends the Account to sign in, then back to this page,
 * when its session ended meanwhile, elsewhere or by expiring: trying again could never succeed.
 */
export function useSignInIfSessionEnded() {
  const queryClient = useQueryClient();
  const router = useRouter();
  // `src/router.tsx` leaves an ended session to the Hub's page: rerunning `_member`'s access check
  // sends the Account to sign in.
  return async (error: Error) => {
    if (hubRefusalReason(error) === RefusalReason.SIGN_IN_REQUIRED) {
      forgetSignedInAccount(queryClient);
      await router.invalidate();
    }
  };
}
