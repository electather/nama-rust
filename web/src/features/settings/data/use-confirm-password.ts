import { useMutation } from "@connectrpc/connect-query";

import { SignInService } from "@/gen/nama/v1/sign_in_pb";

/** Gives this Device Recent Authentication with the User's current password. */
export function useConfirmPassword() {
  return useMutation(SignInService.method.reauthenticate);
}
