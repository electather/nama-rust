import { useMutation } from "@connectrpc/connect-query";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Gives this Device Recent Authentication with the User's password or Account, then calls `onSuccess`. */
export function useReauthenticate(onSuccess: () => void) {
  return useMutation(SignInService.method.reauthenticate, { onSuccess });
}
