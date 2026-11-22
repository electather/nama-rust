import { useMutation } from "@connectrpc/connect-query";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Finishes signing in with an Account the Hub approved, then calls `onSignedIn`. */
export function useSignInWithAccount(onSignedIn: () => Promise<void>) {
  return useMutation(SignInService.method.signInWithAccount, { onSuccess: onSignedIn });
}
