import { callUnaryMethod } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";

import { useIdentityChange } from "@/features/auth/data/use-identity-change";
import { useSignInIfSessionEnded } from "@/features/auth/data/use-sign-in-if-session-ended";
import { AccountService } from "@/gen/nama/hub/v1/account_pb";

/** Deletes the signed-in Account, which signs this browser out, then opens sign-in. */
export function useDeleteAccount() {
  const remove = useIdentityChange({ kind: "account deleted" });
  return useMutation({
    mutationFn: async () => {
      await remove(async (hub) => {
        await callUnaryMethod(hub, AccountService.method.deleteAccount, {});
      });
    },
    onError: useSignInIfSessionEnded(),
  });
}
