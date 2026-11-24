import { useMutation } from "@connectrpc/connect-query";

import { AccountService } from "@/gen/nama/hub/v1/account_pb";

/** Signs an Account in at the Hub with its email and password, then calls `onSignedIn`. */
export function useSignInAccount(onSignedIn: () => Promise<void>) {
  return useMutation(AccountService.method.signIn, { onSuccess: onSignedIn });
}
