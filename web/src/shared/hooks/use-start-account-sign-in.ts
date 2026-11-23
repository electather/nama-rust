import { useMutation } from "@connectrpc/connect-query";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { keepAccountSignIn } from "@/shared/lib/account-sign-in";

/** Starts signing in with an Account, then goes to the Hub to approve it. */
export function useStartAccountSignIn() {
  return useMutation(SignInService.method.startAccountSignIn, {
    onSuccess: ({ url, handle }) => {
      keepAccountSignIn(handle);
      globalThis.location.assign(url);
    },
  });
}
