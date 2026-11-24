import { useMutation } from "@connectrpc/connect-query";
import { useRefreshAccount } from "@hub/features/account/data/account";
import { useRefreshSessions } from "@hub/features/account/data/sessions";
import { AccountService } from "@hub/gen/nama/hub/v1/account_pb";

/**
 * Sets the signed-in Account's password, then asks again whether it has one, and where it's signed
 * in, as that signs it out everywhere else.
 */
export function useSetPassword() {
  const refreshAccount = useRefreshAccount();
  const refreshSessions = useRefreshSessions();
  return useMutation(AccountService.method.setPassword, {
    onSuccess: async () => {
      await Promise.all([refreshAccount(), refreshSessions()]);
    },
  });
}
