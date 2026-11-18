import { useMutation } from "@connectrpc/connect-query";

import { SignInCodeStatus, SignInService } from "@/gen/nama/v1/sign_in_pb";

/**
 * Polls the sign-in a `handle` stands for, calling `onSignedIn` once it's approved.
 *
 * A poll is a mutation, not a query: a signed-in answer hands out the session only once.
 */
export function usePollSignInCode(onSignedIn: () => Promise<void>) {
  return useMutation(SignInService.method.pollSignInCode, {
    onSuccess: async ({ status }) => {
      if (status === SignInCodeStatus.SIGNED_IN) {
        await onSignedIn();
      }
    },
  });
}
