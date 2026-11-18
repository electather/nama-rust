import { useMutation } from "@connectrpc/connect-query";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Signs in with a username and password, then calls `onSignedIn`. */
export function useSignIn(onSignedIn: () => Promise<void>) {
  return useMutation(SignInService.method.signIn, { onSuccess: onSignedIn });
}
