import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { hubRefusalReason } from "@/features/auth/lib/hub-errors";
import { AccountService } from "@/gen/nama/hub/v1/account_pb";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";

/**
 * Signs the Account out of this browser, forgets everything cached for it, and opens sign-in, as it
 * does when the session already ended, elsewhere or by expiring. Signing in goes on to `redirect`.
 */
export function useSignOutAccount(redirect?: string) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const goToSignIn = async () => {
    queryClient.clear();
    await navigate({ to: "/sign-in", search: { redirect } });
  };
  return useMutation(AccountService.method.signOut, {
    onSuccess: goToSignIn,
    onError: async (error) => {
      if (hubRefusalReason(error) === RefusalReason.SIGN_IN_REQUIRED) {
        await goToSignIn();
      }
    },
  });
}
