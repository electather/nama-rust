import { useMutation } from "@connectrpc/connect-query";

import { keepAccountSignIn } from "@/features/auth/lib/account-sign-in";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Starts signing in with an Account, then goes to the Hub to approve it. */
export function useStartAccountSignIn() {
  return useMutation(SignInService.method.startAccountSignIn, {
    onSuccess: ({ url, handle }) => {
      keepAccountSignIn(handle);
      globalThis.location.assign(url);
    },
  });
}
