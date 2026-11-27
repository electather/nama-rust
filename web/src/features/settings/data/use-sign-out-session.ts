import { useMutation } from "@connectrpc/connect-query";

import { useSignInIfSessionEnded } from "@/features/auth/data/use-identity-change";
import { useRefreshAccountSessions } from "@/features/settings/data/use-account-sessions";
import { AccountService } from "@/gen/nama/hub/v1/account_pb";

/** Signs out one of the Account's other sessions, by its id. */
export function useSignOutSession() {
  return useMutation(AccountService.method.signOutSession, {
    onError: useSignInIfSessionEnded(),
    onSettled: useRefreshAccountSessions(),
  });
}
