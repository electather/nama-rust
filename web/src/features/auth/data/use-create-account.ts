import { useMutation } from "@connectrpc/connect-query";

import { AccountService } from "@/gen/nama/hub/v1/account_pb";

/** Creates an Account at the Hub, which signs it in, then calls `onSignedIn`. */
export function useCreateAccount(onSignedIn: () => Promise<void>) {
  return useMutation(AccountService.method.createAccount, { onSuccess: onSignedIn });
}
