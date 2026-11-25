import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { useSignInIfSessionEnded } from "@/features/auth/data/use-sign-in-if-session-ended";
import { useRefreshAccountSessions } from "@/features/settings/data/use-account-sessions";
import { AccountService } from "@/gen/nama/hub/v1/account_pb";
import { signedInAccountQuery } from "@/shared/hooks/access";

/**
 * Sets the signed-in Account's password, then asks again whether it has one, and where it's signed
 * in, as that signs it out everywhere else.
 */
export function useSetAccountPassword() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  const refreshSessions = useRefreshAccountSessions();
  return useMutation(AccountService.method.setPassword, {
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: signedInAccountQuery(transport).queryKey }),
        refreshSessions(),
      ]);
    },
    onError: useSignInIfSessionEnded(),
  });
}
