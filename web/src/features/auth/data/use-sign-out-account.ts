import { callUnaryMethod } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";

import { useIdentityChange } from "@/features/auth/data/use-identity-change";
import { AccountService } from "@/gen/nama/hub/v1/account_pb";

/**
 * Signs the Account out of this browser and opens sign-in, as it does when the session already
 * ended, elsewhere or by expiring. Signing in goes on to `redirect`.
 */
export function useSignOutAccount(redirect?: string) {
  const signOut = useIdentityChange({ kind: "account signed out", goOn: redirect });
  return useMutation({
    mutationFn: async () => {
      await signOut(async (hub) => {
        await callUnaryMethod(hub, AccountService.method.signOut, {});
      });
    },
  });
}
