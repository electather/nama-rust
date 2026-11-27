import { useMutation } from "@connectrpc/connect-query";

import { useSignInIfSessionEnded } from "@/features/auth/data/use-identity-change";
import { useRefreshAccountSessions } from "@/features/settings/data/use-account-sessions";
import { AccountService } from "@/gen/nama/hub/v1/account_pb";

/** Signs out every session of the Account but this browser's. */
export function useSignOutOtherSessions() {
  return useMutation(AccountService.method.signOutOtherSessions, {
    onError: useSignInIfSessionEnded(),
    onSettled: useRefreshAccountSessions(),
  });
}
