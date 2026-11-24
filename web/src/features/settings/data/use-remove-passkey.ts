import { useMutation } from "@connectrpc/connect-query";

import { useSignInIfSessionEnded } from "@/features/auth/data/use-sign-in-if-session-ended";
import { useRefreshPasskeys } from "@/features/settings/data/use-passkeys";
import { AccountService } from "@/gen/nama/hub/v1/account_pb";

/** Removes one of the Account's passkeys by its id, then lists them again, as another tab may have. */
export function useRemovePasskey() {
  return useMutation(AccountService.method.removePasskey, {
    onError: useSignInIfSessionEnded(),
    onSettled: useRefreshPasskeys(),
  });
}
